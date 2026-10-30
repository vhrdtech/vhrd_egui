// The phosphor's compute passes (after common.wgsl): `sweep` rasterises this frame's sweeps into the hit
// counts, `decay` folds them into the persistent density and age and writes the bloom source.

@group(0) @binding(1) var<storage, read_write> hits: array<atomic<u32>>;
@group(0) @binding(2) var<storage, read_write> density: array<f32>;
@group(0) @binding(3) var<storage, read_write> age: array<f32>;
@group(0) @binding(4) var<storage, read> stream: array<f32>;
@group(0) @binding(5) var bright: texture_storage_2d<rgba16float, write>;

// sub-samples per column for the time-domain scenes
const SUB: u32 = 4u;
// salts of the per-sweep random draws
const R_RUNT: u32 = 0x52554e54u;
const R_SPIKE: u32 = 0x53504b45u;
const R_LATE: u32 = 0x4c415445u;
const R_WHERE: u32 = 0x57484552u;
const R_JIT: u32 = 0x4a495454u;
const R_NOISE: u32 = 0x4e4f4953u;
const R_BITS: u32 = 0x42495453u;
const R_HOP: u32 = 0x484f5050u;

fn tanh_step(x: f32) -> f32 {
    return 0.5 + 0.5 * tanh(clamp(x, -20.0, 20.0));
}

// a clock edge: 0 -> 1 in about `rise`, with a damped ring after it
fn edge(x: f32, rise: f32, ring: f32) -> f32 {
    var e = tanh_step(x / (rise * 0.55));
    if x > 0.0 {
        e += ring * exp(-x / (rise * 2.5)) * sin(x / rise * 1.6);
    }
    return e;
}

// Edges further than this many rise times from a sample count as fully done (after) or not yet begun.
const EDGE_REACH: f32 = 40.0;
const MAX_EDGES: i32 = 24;

// One sweep's clock: its edges (time, signed swing) and rare events, worked out once and sampled many times.
struct Sweep {
    t: array<f32, 24>,
    amp: array<f32, 24>,
    count: i32,
    // the level before the first edge
    start: f32,
    // where the spike sits, or < 0 for none
    spike_at: f32,
}

// The clock scene's sweep. The trigger is the rising edge at the centre; edge n has the jitter of n periods.
fn clock_sweep(key: u32) -> Sweep {
    var c: Sweep;
    let period = p.clock.z;
    let swing = p.clock.w;
    let half = period * 0.5;
    let runt = rand(key, R_RUNT) < p.rare.x;
    let late = rand(key, R_LATE) < p.rare.z;
    let reach = min(i32(ceil(0.5 / half)) + 2, (MAX_EDGES - 1) / 2);
    let first = -reach;
    // before the first edge (rising when even) the level is low, else high
    c.start = select(0.5 * swing, -0.5 * swing, (first & 1) == 0);
    c.count = 2 * reach + 1;
    var t2 = 0.0;
    for (var k = 0; k < c.count; k++) {
        let n = first + k;
        let jit = p.clock.y * period * gauss(key ^ R_JIT, u32(n + 64)) * sqrt(f32(abs(n)));
        var t = 0.5 + f32(n) * half + jit;
        if n == 2 {
            t2 = t;
        }
        var amp = swing;
        if late && n == 1 {
            t += p.edge.z * period;
        }
        if runt && (n == 2 || n == 3) {
            amp = 0.4 * swing;
            if n == 3 {
                t = t2 + half * 0.35;
            }
        }
        c.t[k] = t;
        c.amp[k] = select(-amp, amp, (n & 1) == 0);
    }
    c.spike_at = -1.0;
    if rand(key, R_SPIKE) < p.rare.y {
        c.spike_at = 0.5 + half * (1.15 + 0.7 * rand(key, R_WHERE));
    }
    return c;
}

// The clock at screen position u (0..1), in divisions.
fn clock_at(c: ptr<function, Sweep>, u: f32) -> f32 {
    let rise = p.edge.x;
    let ring = p.edge.y;
    var v = (*c).start;
    for (var k = 0; k < (*c).count; k++) {
        let x = u - (*c).t[k];
        var e: f32;
        if x > EDGE_REACH * rise {
            e = 1.0;
        } else if x < -EDGE_REACH * 0.25 * rise {
            e = 0.0;
        } else {
            e = edge(x, rise, ring);
        }
        v += (*c).amp[k] * e;
    }
    if (*c).spike_at >= 0.0 {
        let x = (u - (*c).spike_at) / 0.0012;
        v += 1.1 * p.clock.w * exp(-x * x);
    }
    return v;
}

// The eye scene's sweep: random NRZ bits, the eye centred, crossings at 1/4 and 3/4 of the screen.
fn eye_sweep(key: u32) -> Sweep {
    var c: Sweep;
    let ui = 0.5;
    let amp = 0.5 * p.clock.w;
    let marginal = rand(key, R_RUNT) < p.rare.x;
    var prev = select(-1.0, 1.0, rand(key ^ R_BITS, 0u) < 0.5);
    c.start = prev * amp;
    c.count = 4;
    for (var k = 0; k < 4; k++) {
        let bit = select(-1.0, 1.0, rand(key ^ R_BITS, u32(k + 7)) < 0.5);
        var goal = bit;
        if marginal && k == 1 {
            // a bit that barely makes it: it starts the right way and stops short, inside the eye
            goal = prev + 0.3 * (-prev - prev);
        }
        c.t[k] = 0.5 + (f32(k) - 1.5) * ui + p.clock.y * ui * gauss(key ^ R_JIT, u32(k + 7));
        c.amp[k] = (goal - prev) * amp;
        prev = goal;
    }
    c.spike_at = -1.0;
    if rand(key, R_SPIKE) < p.rare.y {
        c.spike_at = 0.5 + 0.12 * (rand(key, R_WHERE) - 0.5);
        // the spike's sign rides in the swing of the unused last slot
        c.amp[MAX_EDGES - 1] = select(-1.0, 1.0, rand(key, R_WHERE ^ 1u) < 0.5);
    }
    return c;
}

// The eye at u: each transition has a fast part and a slow tail (inter-symbol interference).
fn eye_at(c: ptr<function, Sweep>, u: f32) -> f32 {
    let ui = 0.5;
    let isi = 0.3;
    let tau = 0.3 * ui;
    let rise = 0.1 * ui;
    var v = (*c).start;
    for (var k = 0; k < (*c).count; k++) {
        let x = u - (*c).t[k];
        var resp = (1.0 - isi) * tanh_step(x / (rise * 0.55));
        if x > 0.0 {
            resp += isi * (1.0 - exp(-x / tau));
        }
        v += (*c).amp[k] * resp;
    }
    if (*c).spike_at >= 0.0 {
        let x = (u - (*c).spike_at) / 0.003;
        v += (*c).amp[MAX_EDGES - 1] * 1.3 * 0.5 * p.clock.w * exp(-x * x);
    }
    return v;
}

fn db_to_mw(db: f32) -> f32 {
    return pow(10.0, db * 0.1);
}

// a windowed tone: main lobe plus a phase-noise skirt 80 dB down falling 20 dB per decade
fn tone(d: f32) -> f32 {
    let m = d / 0.0035;
    return exp(-1.5 * m * m) + 1.0e-8 / (1.0 + (d / 0.003) * (d / 0.003));
}

// Pixels per bin of the spectrum; pixels between bins are interpolated (lines between points). A whole
// number, so bins and pixel columns don't beat.
const PX_PER_BIN: f32 = 2.0;

// Noise power of bin i, mean 1: two exponential draws averaged (a little video filtering).
fn noise_bin(key: u32, i: u32) -> f32 {
    let a = -log(max(rand(key ^ R_NOISE, i * 2u), 1.0e-7));
    let b = -log(max(rand(key ^ R_NOISE, i * 2u + 1u), 1.0e-7));
    return 0.5 * (a + b);
}

fn noise_at(key: u32, u: f32) -> f32 {
    let f = clamp(u, 0.0, 1.0) * f32(p.size.x) / PX_PER_BIN;
    let i = u32(f);
    return mix(noise_bin(key, i), noise_bin(key, i + 1u), f - f32(i));
}

// The spectrum scene: power in dBm of bin u for the sweep at `phase` seconds into its burst period, as
// divisions (0 dBm at the top, -120 dBm at the bottom, 15 dB per division).
fn spectrum(u: f32, key: u32, phase: f32, burst: u32) -> f32 {
    let floor = db_to_mw(p.spec0.x) * noise_at(key, u);
    let carrier = db_to_mw(p.spec0.y) * (1.0 + 0.05 * gauss(key, 1u));
    var mw = floor + carrier * tone(u - 0.3);
    mw += db_to_mw(p.spec0.y - 52.0) * tone(u - 0.6);
    mw += db_to_mw(p.spec0.x + 14.0) * tone(u - 0.83);
    if phase < p.spec1.y {
        let hop = u32(phase / p.spec1.z);
        let f = 0.05 + 0.9 * rand(hash(burst ^ p.misc.x) ^ R_HOP, hop);
        let x = (u - f) / 0.006;
        let shape = exp(-x * x * x * x);
        mw += db_to_mw(p.spec0.z) * shape * noise_at(key ^ R_HOP, u);
    }
    let db = 10.0 * log(mw) / log(10.0);
    return db / 15.0 + 4.0;
}

fn streamed(u: f32, s: u32) -> f32 {
    let len = p.misc.z;
    if len < 2u {
        return 0.0;
    }
    let f = clamp(u, 0.0, 1.0) * f32(len - 1u);
    let i = min(u32(f), len - 2u);
    let base = s * len;
    return mix(stream[base + i], stream[base + i + 1u], f - f32(i));
}

@compute @workgroup_size(64, 1, 1)
fn sweep(@builtin(global_invocation_id) gid: vec3<u32>) {
    let w = p.size.x;
    let h = p.size.y;
    let x = gid.x;
    let s = gid.y;
    if x >= w || s >= p.size.z {
        return;
    }
    let key = hash(hash(p.size.w ^ p.misc.x) + s * 0x9e3779b9u);
    let scene = p.misc.y;
    var sub = SUB;
    if scene == 1u || scene == 3u {
        sub = 1u;
    }
    // the sweep's time in its burst period (spectrum)
    var phase = p.time.x + f32(s) * p.time.z;
    var burst = p.misc.w;
    if phase >= p.spec1.x {
        phase -= p.spec1.x;
        burst += 1u;
    }
    var sw: Sweep;
    if scene == 0u {
        sw = clock_sweep(key);
    } else if scene == 2u {
        sw = eye_sweep(key);
    }
    var lo = 1.0e9;
    var hi = -1.0e9;
    for (var j = 0u; j <= sub; j++) {
        let u = (f32(x) + f32(j) / f32(sub)) / f32(w);
        let n = x * sub + j;
        var v: f32;
        if scene == 0u {
            v = clock_at(&sw, u) + p.clock.x * gauss(key ^ R_NOISE, n);
        } else if scene == 1u {
            v = spectrum(u, key, phase, burst);
        } else if scene == 2u {
            v = eye_at(&sw, u) + p.clock.x * gauss(key ^ R_NOISE, n);
        } else {
            v = streamed(u, s);
        }
        let y = (0.5 - v / 8.0) * f32(h);
        lo = min(lo, y);
        hi = max(hi, y);
    }
    if hi < 0.0 || lo >= f32(h) {
        return;
    }
    let y0 = u32(clamp(floor(lo), 0.0, f32(h - 1u)));
    let y1 = u32(clamp(floor(hi), 0.0, f32(h - 1u)));
    for (var y = y0; y <= y1; y++) {
        atomicAdd(&hits[y * w + x], 1u);
    }
}

@compute @workgroup_size(8, 8, 1)
fn decay(@builtin(global_invocation_id) gid: vec3<u32>) {
    let w = p.size.x;
    let h = p.size.y;
    if gid.x >= w || gid.y >= h {
        return;
    }
    let i = gid.y * w + gid.x;
    let n = atomicLoad(&hits[i]);
    atomicStore(&hits[i], 0u);
    let d = density[i] * p.time.w + f32(n);
    density[i] = d;
    var a = min(age[i] + p.time.y, AGE_CAP);
    if n > 0u {
        a = 0.0;
    }
    age[i] = a;
    let c = shade(d, a, p.hdr);
    let b = max(c - vec3<f32>(p.look.z), vec3<f32>(0.0));
    textureStore(bright, vec2<i32>(i32(gid.x), i32(gid.y)), vec4<f32>(b, 1.0));
}
