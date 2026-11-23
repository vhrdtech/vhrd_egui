// What every ve_gpu shader shares: the screen uniform, points to clip space, colours as egui keeps them, the box
// filter that antialiases edges, and egui's rounding to pixels. Prepended to each shader module.
//
// Positions arrive in egui points and are turned into physical pixels here, so coverage is measured per pixel at
// any pixels-per-point. Colours are egui's: premultiplied, gamma space, blended One, OneMinusSrcAlpha.

struct Screen {
    // the target's size in physical pixels
    size_px: vec2<f32>,
    pixels_per_point: f32,
    // 1 when the target is an sRGB format and the output must be linear, as egui's own shader does
    linear: f32,
};
@group(0) @binding(0) var<uniform> screen: Screen;

// A physical pixel position to clip space.
fn ndc(px: vec2<f32>) -> vec4<f32> {
    return vec4<f32>(2.0 * px.x / screen.size_px.x - 1.0, 1.0 - 2.0 * px.y / screen.size_px.y, 0.0, 1.0);
}

// An egui `Color32` packed little-endian: premultiplied RGBA in 0..1.
fn unpack(c: u32) -> vec4<f32> {
    return vec4<f32>(f32(c & 255u), f32((c >> 8u) & 255u), f32((c >> 16u) & 255u), f32(c >> 24u)) / 255.0;
}

// The share of the pixel [d - 0.5, d + 0.5] inside [-half, half].
fn box1(d: f32, half: f32) -> f32 {
    return max(0.0, min(d + 0.5, half) - max(d - 0.5, -half));
}

// Rust's `f32::round`: half away from zero (WGSL's `round` is half to even).
fn round_away(v: f32) -> f32 {
    return sign(v) * floor(abs(v) + 0.5);
}

// Rounded to the nearest pixel centre.
fn round_to_centre(v: f32) -> f32 {
    return round_away(v - 0.5) + 0.5;
}

// egui's `Stroke::round_center_to_pixel`: a line of odd or at most one pixel's width sits on pixel centres, an even
// one on pixel edges.
fn round_line(width_px: f32, v: f32) -> f32 {
    let w = i32(round_away(width_px));
    if width_px <= 1.0 || w % 2 == 1 {
        return round_to_centre(v);
    }
    return round_away(v);
}

// The coverage of a segment from `a` to `b`, `w` wide, with butt ends half a pixel past each end point (egui's
// feathering), dashed when `dash` is above zero; all in pixels.
fn segment_cover(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, w: f32, dash: f32, gap: f32) -> f32 {
    let d = b - a;
    let len = length(d);
    if len <= 0.0 {
        return 0.0;
    }
    let u = d / len;
    let t = dot(p - a, u);
    let n = dot(p - a, vec2<f32>(-u.y, u.x));
    var along = box1(t - len * 0.5, len * 0.5 + 0.5);
    if dash > 0.0 {
        let m = t - floor(t / (dash + gap)) * (dash + gap);
        along = min(along, box1(m - dash * 0.5, dash * 0.5) + box1(m - (dash + gap) - dash * 0.5, dash * 0.5));
    }
    return along * box1(n, w * 0.5);
}

// Two triangles over the unit square: 0 1 2, 2 1 3.
fn corner(i: u32) -> vec2<f32> {
    var k = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
    );
    return k[i];
}

fn linear_from_gamma(c: vec3<f32>) -> vec3<f32> {
    let cutoff = c < vec3<f32>(0.04045);
    let lower = c / vec3<f32>(12.92);
    let higher = pow((c + vec3<f32>(0.055)) / vec3<f32>(1.055), vec3<f32>(2.4));
    return select(higher, lower, cutoff);
}

// A premultiplied gamma colour as the target wants it.
fn output(c: vec4<f32>) -> vec4<f32> {
    if screen.linear > 0.5 {
        return vec4<f32>(linear_from_gamma(c.rgb), c.a);
    }
    return c;
}
