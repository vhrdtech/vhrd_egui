// The phosphor view (after common.wgsl): one triangle over the paint callback's viewport, each fragment reads
// its pixel's density and age, tone maps it for its side of the split, adds bloom and the graticule.

@group(0) @binding(1) var<storage, read> density: array<f32>;
@group(0) @binding(2) var<storage, read> age: array<f32>;
@group(0) @binding(3) var bloom_tex: texture_2d<f32>;

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

// 1 on the graticule's lines and ticks, else 0
fn graticule(q: vec2<i32>, size: vec2<f32>) -> f32 {
    let f = vec2<f32>(q) + 0.5;
    let div = vec2<f32>(10.0, 8.0);
    let cell = size / div;
    // major lines: 10 x 8 divisions, dotted (every other pixel pair) so they stay faint
    let g = f / cell;
    let near = abs(g - round(g)) * cell;
    let dotted = ((q.x + q.y) / 2) % 2 == 0;
    var on = 0.0;
    if (near.x < 0.5 || near.y < 0.5) && dotted {
        on = 0.6;
    }
    // the centre axes, solid, with ticks every fifth of a division
    let c = size * 0.5;
    let tick = cell / 5.0;
    let t = f / tick;
    let tnear = abs(t - round(t)) * tick;
    if abs(f.x - c.x) < 0.5 || abs(f.y - c.y) < 0.5 {
        on = 1.0;
    }
    if (abs(f.y - c.y) < 4.0 && tnear.x < 0.5) || (abs(f.x - c.x) < 4.0 && tnear.y < 0.5) {
        on = 1.0;
    }
    // the border
    if f.x < 1.0 || f.y < 1.0 || f.x > size.x - 1.0 || f.y > size.y - 1.0 {
        on = 1.0;
    }
    return on;
}

@fragment
fn fs_main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let w = i32(p.size.x);
    let h = i32(p.size.y);
    let q = vec2<i32>(floor(pos.xy - p.view.xy));
    var c = vec3<f32>(0.0);
    if q.x >= 0 && q.y >= 0 && q.x < w && q.y < h {
        let i = u32(q.y * w + q.x);
        let d = density[i];
        let a = age[i];
        let mode = p.view.w;
        let hdr_side = mode < 0.5 || (mode > 1.5 && f32(q.x) < p.view.z);
        if hdr_side {
            c = shade(d, a, p.hdr);
            if p.look.y > 0.0 {
                c += textureLoad(bloom_tex, q, 0).rgb * p.look.y;
            }
            c *= p.hdr.w;
        } else {
            c = shade(d, a, p.sdr) * p.sdr.w;
        }
        let g = graticule(q, vec2<f32>(f32(w), f32(h))) * p.look.x;
        c = max(c, vec3<f32>(g));
    }
    c = min(c, vec3<f32>(p.output.y));
    if p.output.x < 0.5 {
        c = encode3(c);
    }
    return vec4<f32>(c, 1.0);
}
