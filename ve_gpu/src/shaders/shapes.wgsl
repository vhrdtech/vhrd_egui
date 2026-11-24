// The shape batch: rects with corners and an inner stroke, segments with dashes, diamonds, hatching and rects
// filled with a gradient, one
// instance each, antialiased by a one pixel box filter of each edge (what egui's feathering gives).

const KIND_RECT: u32 = 0u;
const KIND_SEGMENT: u32 = 1u;
const KIND_DIAMOND: u32 = 2u;
const KIND_HATCH: u32 = 3u;
const KIND_GRADIENT: u32 = 4u;
const KIND_MASK: u32 = 255u;
// Round to pixels as egui's tessellator does.
const FLAG_SNAP: u32 = 256u;

struct ShapeIn {
    // rect, segment, gradient: x0 y0 x1 y1; diamond: centre x, y, half size; hatch: the hatched rect
    @location(0) a: vec4<f32>,
    // the shape's clip rect
    @location(1) clip: vec4<f32>,
    // rect: stroke width, corner radius; segment: width, 0, dash, gap; diamond: stroke width;
    // hatch: line width, period, anchor x, anchor y
    @location(2) p: vec4<f32>,
    // kind and flags, fill (a gradient's start), stroke (its end), hatch slant (f32 bits) or a gradient's axis (1: y)
    @location(3) c: vec4<u32>,
};

struct ShapeOut {
    @builtin(position) pos: vec4<f32>,
    // `a`, `clip` and `p` in pixels, after rounding
    @location(0) @interpolate(flat) a: vec4<f32>,
    @location(1) @interpolate(flat) clip: vec4<f32>,
    @location(2) @interpolate(flat) p: vec4<f32>,
    @location(3) @interpolate(flat) c: vec4<u32>,
    @location(4) px: vec2<f32>,
};

fn round4(v: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(round_away(v.x), round_away(v.y), round_away(v.z), round_away(v.w));
}

// egui's line segment on pixels: an axis-aligned one moves to pixel centres (or edges, for even widths) across, and
// an undashed one is shortened by a quarter pixel at each end, then its ends rounded to pixel centres.
fn snap_segment(a_in: vec4<f32>, width: f32, dashed: bool) -> vec4<f32> {
    var a = a_in;
    if a.x == a.z {
        let x = round_line(width, a.x);
        a.x = x;
        a.z = x;
        if !dashed {
            let s = select(-0.25, 0.25, a.y < a.w);
            a.y = round_to_centre(a.y + s);
            a.w = round_to_centre(a.w - s);
        }
    }
    if a.y == a.w {
        let y = round_line(width, a.y);
        a.y = y;
        a.w = y;
        if !dashed {
            let s = select(-0.25, 0.25, a.x < a.z);
            a.x = round_to_centre(a.x + s);
            a.z = round_to_centre(a.z - s);
        }
    }
    return a;
}

@vertex
fn vs_shape(@builtin(vertex_index) vi: u32, s: ShapeIn) -> ShapeOut {
    let k = screen.pixels_per_point;
    let kind = s.c.x & KIND_MASK;
    let snap = (s.c.x & FLAG_SNAP) != 0u;
    var a = s.a * k;
    var p = s.p * k;
    var lo: vec2<f32>;
    var hi: vec2<f32>;
    switch kind {
        case KIND_RECT, KIND_GRADIENT: {
            if snap {
                a = round4(a);
            }
            lo = a.xy - vec2<f32>(1.0);
            hi = a.zw + vec2<f32>(1.0);
        }
        case KIND_SEGMENT: {
            if snap {
                a = snap_segment(a, p.x, p.z > 0.0);
            }
            let m = p.x * 0.5 + 1.5;
            lo = min(a.xy, a.zw) - vec2<f32>(m);
            hi = max(a.xy, a.zw) + vec2<f32>(m);
        }
        case KIND_DIAMOND: {
            let m = a.z + p.x * 0.5 + 1.5;
            lo = a.xy - vec2<f32>(m);
            hi = a.xy + vec2<f32>(m);
        }
        default: {
            // a hatch: whole pixels of its rect, its anchor on a pixel
            a = round4(a);
            p = vec4<f32>(p.x, p.y, round_away(p.z), round_away(p.w));
            lo = a.xy;
            hi = a.zw;
        }
    }
    let clip = s.clip * k;
    lo = max(lo, clip.xy);
    hi = max(min(hi, clip.zw), lo);
    let px = mix(lo, hi, corner(vi));
    var o: ShapeOut;
    o.pos = ndc(px);
    o.a = a;
    o.clip = clip;
    o.p = p;
    o.c = s.c;
    o.px = px;
    return o;
}

// Signed distance to a box of half size `b` with corner radius `r`, centred at 0.
fn rounded_box(p: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - b + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

// A box's coverage: the exact box filter when square cornered, a distance ramp when rounded.
fn box_cover(p: vec2<f32>, lo: vec2<f32>, hi: vec2<f32>, r: f32) -> f32 {
    let c = (lo + hi) * 0.5;
    let h = max((hi - lo) * 0.5, vec2<f32>(0.0));
    if r <= 0.0 {
        return box1(p.x - c.x, h.x) * box1(p.y - c.y, h.y);
    }
    return clamp(0.5 - rounded_box(p - c, h, min(r, min(h.x, h.y))), 0.0, 1.0);
}

@fragment
fn fs_shape(i: ShapeOut) -> @location(0) vec4<f32> {
    let p = i.px;
    if p.x < i.clip.x || p.y < i.clip.y || p.x > i.clip.z || p.y > i.clip.w {
        discard;
    }
    let fill = unpack(i.c.y);
    let stroke = unpack(i.c.z);
    var out = vec4<f32>(0.0);
    switch i.c.x & KIND_MASK {
        case KIND_RECT: {
            let outer = box_cover(p, i.a.xy, i.a.zw, i.p.y);
            let w = i.p.x;
            if w > 0.0 {
                let inner = box_cover(p, i.a.xy + vec2<f32>(w), i.a.zw - vec2<f32>(w), max(i.p.y - w, 0.0));
                out = fill * inner + stroke * (outer - inner);
            } else {
                out = fill * outer;
            }
        }
        case KIND_SEGMENT: {
            out = stroke * segment_cover(p, i.a.xy, i.a.zw, i.p.x, i.p.z, i.p.w);
        }
        case KIND_GRADIENT: {
            // from the fill at the rect's start to the stroke colour at its end, along x or y
            let t = select(
                clamp((p.x - i.a.x) / max(i.a.z - i.a.x, 1e-6), 0.0, 1.0),
                clamp((p.y - i.a.y) / max(i.a.w - i.a.y, 1e-6), 0.0, 1.0),
                i.c.w == 1u,
            );
            out = mix(fill, stroke, t) * box_cover(p, i.a.xy, i.a.zw, 0.0);
        }
        case KIND_DIAMOND: {
            let q = abs(p - i.a.xy);
            // distance to the edge, across it
            let sd = (q.x + q.y - i.a.z) * 0.70710678;
            let inside = clamp(0.5 - sd, 0.0, 1.0);
            let edge = box1(sd, i.p.x * 0.5);
            out = stroke * edge + fill * inside * (1.0 - edge);
        }
        default: {
            // bands of whole pixels: `x + k y` of the pixel centre within the width past a line, the lines half a
            // pixel before the anchor and every period from it
            if p.x < i.a.x || p.y < i.a.y || p.x > i.a.z || p.y > i.a.w {
                discard;
            }
            let k = bitcast<f32>(i.c.w);
            let s = (p.x + k * p.y) - (i.p.z + k * i.p.w - 0.5);
            let m = s - floor(s / i.p.y) * i.p.y;
            out = stroke * select(0.0, 1.0, m < i.p.x);
        }
    }
    // nothing to add: a premultiplied colour of all zeros (an additive one keeps its alpha at zero)
    if all(out == vec4<f32>(0.0)) {
        discard;
    }
    return output(out);
}
