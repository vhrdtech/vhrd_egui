// Shared by the phosphor shaders: the parameter block, random numbers and the tone map (mirrors tonemap.rs:
// change both together).

struct Params {
    // w, h (pixels of the hit buffer), sweeps this frame, frame number
    size: vec4<u32>,
    // seed, scene (0 clock, 1 spectrum, 2 eye, 3 stream), stream sweep length, burst index
    misc: vec4<u32>,
    // burst phase at the frame's start (s), dt (s), seconds between sweeps, decay factor k
    time: vec4<f32>,
    // noise (div), jitter (of a period), period (screen widths), swing (div)
    clock: vec4<f32>,
    // rise (screen widths), ringing (of the swing), late_by (of a period), -
    edge: vec4<f32>,
    // runt, spike, late rates per sweep, -
    rare: vec4<f32>,
    // floor, carrier, burst power (dBm), -
    spec0: vec4<f32>,
    // burst period, burst length, hop dwell (s), -
    spec1: vec4<f32>,
    // full density, intensity, cutoff (hits), age span (s)
    tone: vec4<f32>,
    // HDR side: peak, ghost, palette, scale
    hdr: vec4<f32>,
    // SDR side: peak, ghost, palette, scale
    sdr: vec4<f32>,
    // view origin in framebuffer pixels (x, y), split position (pixels from the left), mode (0 HDR, 1 SDR, 2 split)
    view: vec4<f32>,
    // graticule, bloom strength, bloom threshold, bloom radius (px)
    look: vec4<f32>,
    // phosphor colour (linear rgb), -
    color: vec4<f32>,
    // 1 = write linear (an *Srgb target encodes), largest value the output shows, -, -
    output: vec4<f32>,
}

@group(0) @binding(0) var<uniform> p: Params;

const AGE_CAP: f32 = 1.0e6;

// lowbias32 (Chris Wellons): a good 32-bit integer hash
fn hash(v: u32) -> u32 {
    var x = v;
    x ^= x >> 16u;
    x *= 0x7feb352du;
    x ^= x >> 15u;
    x *= 0x846ca68bu;
    x ^= x >> 16u;
    return x;
}

// uniform in [0, 1), 24 bits
fn rand(key: u32, n: u32) -> f32 {
    return f32(hash(key ^ hash(n)) >> 8u) * (1.0 / 16777216.0);
}

// standard normal (Box-Muller)
fn gauss(key: u32, n: u32) -> f32 {
    let u1 = max(rand(key, n * 2u), 1.0e-7);
    let u2 = rand(key, n * 2u + 1u);
    return sqrt(-2.0 * log(u1)) * cos(6.2831853 * u2);
}

fn level(d: f32, full: f32, intensity: f32) -> f32 {
    if full <= 0.0 || d <= 0.0 {
        return 0.0;
    }
    return clamp(log(1.0 + d * intensity) / log(1.0 + full), 0.0, 1.0);
}

fn visibility(d: f32, cutoff: f32) -> f32 {
    return smoothstep(cutoff, 4.0 * cutoff, d);
}

fn luminance(lv: f32, ghost_in: f32, peak_in: f32) -> f32 {
    let ghost = max(ghost_in, 1.0e-4);
    let peak = max(peak_in, ghost);
    return ghost * pow(peak / ghost, clamp(lv, 0.0, 1.0));
}

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn unit_luma(c: vec3<f32>) -> vec3<f32> {
    let y = luma(c);
    if y <= 0.0 {
        return vec3<f32>(0.0);
    }
    return c / y;
}

fn fit_peak(c: vec3<f32>, peak: f32) -> vec3<f32> {
    let m = max(c.r, max(c.g, c.b));
    if m > peak && m > 0.0 {
        return c * (peak / m);
    }
    return c;
}

fn phosphor(lv: f32, lum: f32, color: vec3<f32>) -> vec3<f32> {
    let white = smoothstep(0.75, 1.0, lv) * 0.7;
    return mix(unit_luma(color), vec3<f32>(1.0), white) * lum;
}

fn decode(x: f32) -> f32 {
    let a = abs(x);
    var d: f32;
    if a <= 0.04045 {
        d = a / 12.92;
    } else {
        d = pow((a + 0.055) / 1.055, 2.4);
    }
    return select(d, -d, x < 0.0);
}

fn encode(x: f32) -> f32 {
    let a = abs(x);
    var e: f32;
    if a <= 0.0031308 {
        e = a * 12.92;
    } else {
        e = 1.055 * pow(a, 1.0 / 2.4) - 0.055;
    }
    return select(e, -e, x < 0.0);
}

fn decode3(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(decode(c.r), decode(c.g), decode(c.b));
}

fn encode3(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(encode(c.r), encode(c.g), encode(c.b));
}

fn temperature(lv_in: f32, peak: f32) -> vec3<f32> {
    let l = clamp(lv_in, 0.0, 1.0);
    var stops = array<vec4<f32>, 7>(
        vec4<f32>(0.0, 0.10, 0.10, 0.55),
        vec4<f32>(0.22, 0.0, 0.45, 1.0),
        vec4<f32>(0.42, 0.0, 0.85, 0.75),
        vec4<f32>(0.60, 0.35, 1.0, 0.1),
        vec4<f32>(0.76, 1.0, 0.9, 0.0),
        vec4<f32>(0.90, 1.0, 0.3, 0.05),
        vec4<f32>(1.0, 1.0, 1.0, 1.0),
    );
    var c = stops[6].yzw;
    for (var i = 0; i < 6; i++) {
        let a = stops[i];
        let b = stops[i + 1];
        if l <= b.x {
            c = mix(a.yzw, b.yzw, (l - a.x) / (b.x - a.x));
            break;
        }
    }
    return decode3(c) * pow(max(peak, 1.0), l * l);
}

const AGE_FRESH: vec3<f32> = vec3<f32>(0.75, 0.95, 1.0);
const AGE_OLD: vec3<f32> = vec3<f32>(0.5, 0.12, 1.0);

fn age_palette(lv: f32, lum: f32, age_s: f32, span: f32) -> vec3<f32> {
    let t = sqrt(clamp(age_s / max(span, 1.0e-3), 0.0, 1.0));
    let hue = mix(unit_luma(AGE_FRESH), unit_luma(AGE_OLD), smoothstep(0.0, 1.0, t));
    let white = smoothstep(0.8, 1.0, lv) * 0.5;
    return mix(hue, vec3<f32>(1.0), white) * lum;
}

// The colour of a pixel of density d and age a on one side (peak, ghost, palette, scale): linear light.
fn shade(d: f32, a: f32, side: vec4<f32>) -> vec3<f32> {
    let vis = visibility(d, p.tone.z);
    if vis <= 0.0 {
        return vec3<f32>(0.0);
    }
    let lv = level(d, p.tone.x, p.tone.y);
    let lum = luminance(lv, side.y, side.x);
    var c: vec3<f32>;
    if side.z < 0.5 {
        c = phosphor(lv, lum, p.color.rgb);
    } else if side.z < 1.5 {
        c = temperature(lv, side.x);
    } else {
        c = age_palette(lv, lum, a, p.tone.w);
    }
    return fit_peak(c, side.x) * vis;
}
