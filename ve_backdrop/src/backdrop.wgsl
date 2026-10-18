// The backdrop per pixel: find the grid cell the pixel is in, work out that cell's mark (the same math as
// `Scene::sample` in field.rs, keep the two in step) and how much of the pixel it covers.

const MAX_LIT: i32 = 32;
const MAX_RINGS: i32 = 8;

struct Globals {
    // min.xy, max.xy of the covered area, in points
    area: vec4<f32>,
    // spacing, size, mark (0 dots, 1 squares), pixels per point
    grid: vec4<f32>,
    // opacity, ambient, wave number, phase
    base: vec4<f32>,
    // pointer.xy, lens on (0 / 1), lens radius
    pointer: vec4<f32>,
    // push (peak-normalized), grow, light, background opacity
    lens: vec4<f32>,
    // unused, unused, unused, ring count
    ripple: vec4<f32>,
    // reach, light, grow, lit count
    glow: vec4<f32>,
    // linear (sRGB-aware) framebuffer (0 / 1), unused
    frame: vec4<f32>,
    // straight gamma-space colours
    bg_color: vec4<f32>,
    mark_color: vec4<f32>,
    glow_color: vec4<f32>,
    pointer_color: vec4<f32>,
    // min.xy, max.xy per lit frame
    lit_rect: array<vec4<f32>, 32>,
    // corner radius, strength per lit frame
    lit_par: array<vec4<f32>, 32>,
    // origin.xy, radius, width per ring
    rings: array<vec4<f32>, 8>,
    // fade, light, grow, push per ring
    ring_par: array<vec4<f32>, 8>,
    // straight gamma-space colour per ring
    ring_color: array<vec4<f32>, 8>,
};

@group(0) @binding(0) var<uniform> g: Globals;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    // One triangle over the whole viewport (egui sets it to the callback's rectangle).
    let x = f32(i32(index & 1u) * 4 - 1);
    let y = f32(i32(index >> 1u) * 4 - 1);
    return vec4<f32>(x, y, 0.0, 1.0);
}

fn sd_round_rect(p: vec2<f32>, r: vec4<f32>, corner: f32) -> f32 {
    let half = (r.zw - r.xy) * 0.5;
    let rad = clamp(corner, 0.0, max(min(half.x, half.y), 0.0));
    let d = abs(p - (r.xy + r.zw) * 0.5) - half + vec2<f32>(rad);
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0) - rad;
}

fn frame_light(p: vec2<f32>) -> f32 {
    var sum = 0.0;
    let count = min(i32(g.glow.w), MAX_LIT);
    for (var i = 0; i < count; i++) {
        let q = max(sd_round_rect(p, g.lit_rect[i], g.lit_par[i].x), 0.0) / g.glow.x;
        sum += g.lit_par[i].y * exp(-q * q);
    }
    return 1.0 - exp(-sum);
}

fn linear_from_gamma(srgb: vec3<f32>) -> vec3<f32> {
    let lower = srgb / vec3<f32>(12.92);
    let higher = pow((srgb + vec3<f32>(0.055)) / vec3<f32>(1.055), vec3<f32>(2.4));
    return select(higher, lower, srgb < vec3<f32>(0.04045));
}

@fragment
fn fs_main(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let ppp = g.grid.w;
    let spacing = g.grid.x;
    let p = frag.xy / ppp;
    let c = g.area.xy + (floor((p - g.area.xy) / spacing) + vec2<f32>(0.5)) * spacing;

    let wave = sin(c.x * g.base.z + g.base.w) * sin(c.y * g.base.z * 0.8 - g.base.w * 0.7);
    let base = max(g.base.x * (1.0 + g.base.y * wave), 0.0);

    var grow = 0.0;
    var offset = vec2<f32>(0.0);
    var pointer_light = 0.0;
    var ring_light = 0.0;
    var ring_rgba = vec4<f32>(0.0);

    if g.pointer.z > 0.5 {
        let d = (c - g.pointer.xy) / g.pointer.w;
        let f = exp(-dot(d, d));
        offset += d * f * g.lens.x;
        grow += g.lens.y * f;
        pointer_light += g.lens.z * f;
    }

    let rings = min(i32(g.ripple.w), MAX_RINGS);
    for (var i = 0; i < rings; i++) {
        let ring = g.rings[i];
        let par = g.ring_par[i];
        let d = c - ring.xy;
        let dist = length(d);
        let q = (dist - ring.z) / max(ring.w, 0.001);
        let r = exp(-q * q) * par.x;
        let light = par.y * r;
        ring_light += light;
        ring_rgba += g.ring_color[i] * light;
        grow += par.z * r;
        if dist > 0.001 {
            offset += d / dist * (r * par.w);
        }
    }

    let light = frame_light(c);
    let glow_light = g.glow.y * light;
    grow += g.glow.z * light;

    let half = clamp(g.grid.y * 0.5 * (1.0 + grow), 0.0, spacing * 0.5);
    let max_offset = max(spacing * 0.5 - half, 0.0);
    let len = length(offset);
    if len > max_offset {
        offset *= max_offset / max(len, 0.000001);
    }

    let total = base + pointer_light + glow_light + ring_light;
    let color = (g.mark_color * base + g.pointer_color * pointer_light + g.glow_color * glow_light
        + ring_rgba) / max(total, 0.000001);
    let alpha = clamp(total, 0.0, 1.0) * color.a;

    let center = c + offset;
    let d = abs(p - center);
    // A mark thinner than a pixel dims instead of staying half a pixel bright.
    var cover = clamp(0.5 - (length(d) - half) * ppp, 0.0, 1.0) * clamp(2.0 * half * ppp, 0.0, 1.0);
    if g.grid.z > 0.5 {
        // Squares have their edges on pixel boundaries (as egui rounds rectangles), so they stay crisp
        // and read as squares even when small.
        let lo = round((center - vec2<f32>(half)) * ppp);
        let hi = max(round((center + vec2<f32>(half)) * ppp), lo + vec2<f32>(1.0));
        let inside = step(lo.x, frag.x) * step(frag.x, hi.x) * step(lo.y, frag.y) * step(frag.y, hi.y);
        cover = inside * clamp(2.0 * half * ppp, 0.0, 1.0);
    }

    let a = cover * alpha;
    let bg_a = g.bg_color.a * g.lens.w;
    // Premultiplied, gamma space: what egui's own pipeline blends with.
    var rgb = color.rgb * a + g.bg_color.rgb * bg_a * (1.0 - a);
    let out_a = a + bg_a * (1.0 - a);
    if g.frame.x > 0.5 {
        rgb = linear_from_gamma(rgb);
    }
    return vec4<f32>(rgb, out_a);
}
