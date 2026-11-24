// Range columns: per column of pixels a value's lowest, highest and mean, mapped to the screen here. The columns
// follow x or y (`run.axis.x`); along and across below are pixels along that axis and across it. Three parts,
// each optional: the band of whole pixels from the lowest to the highest, the two edges along the lowest and the
// highest values (egui's line of their width with feathering, mitred at joins), and the mean line.

// One run of columns: its clip, its value map and colours.
struct Run {
    // the run's clip rect, points
    clip: vec4<f32>,
    // across = map.z + (value - map.x) * map.y, points; map.w: the mean line's width, points
    map: vec4<f32>,
    // band, mean and edge colours, the run's column count
    colours: vec4<u32>,
    // x: 1 when the columns follow y; y: the edges' width, points
    axis: vec4<f32>,
};
@group(1) @binding(0) var<uniform> run: Run;

// Where a value lands across the axis, pixels.
fn across(v: f32) -> f32 {
    return (run.map.z + (v - run.map.x) * run.map.y) * screen.pixels_per_point;
}

// The pixel at `along` and `across` the run's axis.
fn place(along: f32, across: f32) -> vec2<f32> {
    if run.axis.x > 0.5 {
        return vec2<f32>(across, along);
    }
    return vec2<f32>(along, across);
}

fn outside(p: vec2<f32>) -> bool {
    let c = run.clip * screen.pixels_per_point;
    return p.x < c.x || p.y < c.y || p.x > c.z || p.y > c.w;
}

struct BandOut {
    @builtin(position) pos: vec4<f32>,
};

// A pixel column of the band, (along, across from, across to, width) in points as the CPU folded it: whole pixels
// from the one the first end falls in to the one the second does, at least one pixel long.
@vertex
fn vs_band(@builtin(vertex_index) vi: u32, @location(0) col: vec4<f32>) -> BandOut {
    let k = screen.pixels_per_point;
    let x0 = round_away(col.x * k);
    let x1 = max(round_away((col.x + col.w) * k), x0 + 1.0);
    let y0 = floor(col.y * k);
    let y1 = max(ceil(col.z * k), y0 + 1.0);
    let c = corner(vi);
    var o: BandOut;
    o.pos = ndc(place(mix(x0, x1, c.x), mix(y0, y1, c.y)));
    return o;
}

@fragment
fn fs_band(i: BandOut) -> @location(0) vec4<f32> {
    if outside(i.pos.xy) {
        discard;
    }
    return output(unpack(run.colours.x));
}

struct MeanOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) px: vec2<f32>,
    @location(1) @interpolate(flat) ab: vec4<f32>,
};

fn point(c: vec4<f32>, v: f32) -> vec2<f32> {
    return place(c.x * screen.pixels_per_point, across(v));
}

// A piece of the mean line between a column and the next.
@vertex
fn vs_mean(@builtin(vertex_index) vi: u32, @location(0) c0: vec4<f32>, @location(1) c1: vec4<f32>) -> MeanOut {
    let a = point(c0, c0.w);
    let b = point(c1, c1.w);
    let m = run.map.w * screen.pixels_per_point * 0.5 + 1.5;
    let px = mix(min(a, b) - vec2<f32>(m), max(a, b) + vec2<f32>(m), corner(vi));
    var o: MeanOut;
    o.pos = ndc(px);
    o.px = px;
    o.ab = vec4<f32>(a, b);
    return o;
}

@fragment
fn fs_mean(i: MeanOut) -> @location(0) vec4<f32> {
    if outside(i.pos.xy) {
        discard;
    }
    let c = segment_cover(i.px, i.ab.xy, i.ab.zw, run.map.w * screen.pixels_per_point, 0.0, 0.0);
    if c <= 0.0 {
        discard;
    }
    return output(unpack(run.colours.y) * c);
}

// An edge as egui tessellates an open line of the run's width with one pixel of feathering (`epaint::tessellator`,
// `Path::add_open_points` and `stroke_path`), mitred at joins, a corner sharper than a right angle cut in two
// normals with a strip between them. A line wider than 0.9 pixels has a core of its colour a pixel narrower than
// it, fading out over a pixel either side, and a pixel of fade past each end. A thinner one is a ridge of its colour
// times its width in pixels, fading out over a pixel either side, without ends. An instance is the piece from
// column k to k + 1, with columns k - 1 and k + 2 for its joins: sixteen triangles, unused ones empty.
struct EdgeOut {
    @builtin(position) pos: vec4<f32>,
    // the share of the line's colour, interpolated as egui interpolates its vertex colours
    @location(0) t: f32,
};

fn rot90(v: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(v.y, -v.x);
}

// emath's `normalized`: a zero vector stays zero.
fn normalized(v: vec2<f32>) -> vec2<f32> {
    let len = length(v);
    if len <= 0.0 {
        return v;
    }
    return v / len;
}

// The normals a join gives the piece arriving (xy) and the piece leaving (zw), and whether the corner was cut.
struct Join {
    n: vec4<f32>,
    cut: bool,
};

fn join(n0: vec2<f32>, n1: vec2<f32>) -> Join {
    let normal = (n0 + n1) / 2.0;
    let length_sq = dot(normal, normal);
    var j: Join;
    if length_sq < 0.5 {
        let centre = normalized(normal);
        let n0c = (n0 + centre) / 2.0;
        let n1c = (n1 + centre) / 2.0;
        j.n = vec4<f32>(n0c / dot(n0c, n0c), n1c / dot(n1c, n1c));
        j.cut = true;
    } else {
        j.n = vec4<f32>(normal / length_sq, normal / length_sq);
        j.cut = false;
    }
    return j;
}

// Vertex `vi` of piece `k` through the points `a` (column k - 1), `b`, `c` and `d` (column k + 2) of `n` columns.
fn edge(vi: u32, k: u32, n: u32, a: vec2<f32>, b: vec2<f32>, c: vec2<f32>, d: vec2<f32>) -> EdgeOut {
    let first = k == 0u;
    let last = k + 2u == n;
    let along = normalized(c - b);
    let nb = rot90(along);
    // the normals at b: arriving from a, leaving towards c; at c: arriving from b, leaving towards d
    var jb = Join(vec4<f32>(nb, nb), false);
    if !first {
        jb = join(rot90(normalized(b - a)), nb);
    }
    var jc = Join(vec4<f32>(nb, nb), false);
    if !last {
        jc = join(nb, rot90(normalized(d - c)));
    }
    // egui's radii from the line's middle, pixels: to the end of the fade and to the end of the core; a thin line
    // is a ridge, its colour scaled down
    let w = run.axis.y * screen.pixels_per_point;
    let thin = w <= 0.9;
    var outer = 1.0;
    var inner = 0.0;
    var alpha = w;
    if !thin {
        outer = 0.5 * (w + 1.0);
        inner = 0.5 * (w - 1.0);
        alpha = 1.0;
    }
    // three rings of four vertices (outer and inner on the normal's side, inner and outer on the other): the strip's
    // start at b for a cut corner (0), the piece's start at b (1) and its end at c (2); a thick line's outer vertices
    // at its ends are pushed a pixel past them
    var tables = array<u32, 48>(
        // the start's fade
        4u, 5u, 6u, 4u, 6u, 7u,
        // the strip of a cut corner at b
        0u, 1u, 4u, 1u, 4u, 5u, 1u, 2u, 5u, 2u, 5u, 6u, 2u, 3u, 6u, 3u, 6u, 7u,
        // the piece
        4u, 5u, 8u, 5u, 8u, 9u, 5u, 6u, 9u, 6u, 9u, 10u, 6u, 7u, 10u, 7u, 10u, 11u,
        // the end's fade
        8u, 9u, 10u, 8u, 10u, 11u,
    );
    let tri = vi / 3u;
    let used = (tri < 2u && first && !thin) || (tri >= 2u && tri < 8u && jb.cut && !first) || (tri >= 8u && tri < 14u)
        || (tri >= 14u && last && !thin);
    var o: EdgeOut;
    if !used {
        o.pos = vec4<f32>(0.0, 0.0, 0.0, 1.0);
        o.t = 0.0;
        return o;
    }
    let v = tables[vi];
    let ring = v / 4u;
    let slot = v % 4u;
    var base = b;
    var normal = jb.n.zw;
    var push = vec2<f32>(0.0);
    if ring == 0u {
        normal = jb.n.xy;
    } else if ring == 1u {
        if first && !thin {
            push = -along;
        }
    } else {
        base = c;
        normal = jc.n.xy;
        if last && !thin {
            push = along;
        }
    }
    var offsets = array<f32, 4>(outer, inner, -inner, -outer);
    var p = base + normal * offsets[slot];
    if slot == 0u || slot == 3u {
        p += push;
        o.t = 0.0;
    } else {
        o.t = alpha;
    }
    o.pos = ndc(p);
    return o;
}

// A piece of the line along the highest values.
@vertex
fn vs_edge_max(
    @builtin(vertex_index) vi: u32,
    @builtin(instance_index) k: u32,
    @location(0) a: vec4<f32>,
    @location(1) b: vec4<f32>,
    @location(2) c: vec4<f32>,
    @location(3) d: vec4<f32>,
) -> EdgeOut {
    return edge(vi, k, run.colours.w, point(a, a.z), point(b, b.z), point(c, c.z), point(d, d.z));
}

// A piece of the line along the lowest values.
@vertex
fn vs_edge_min(
    @builtin(vertex_index) vi: u32,
    @builtin(instance_index) k: u32,
    @location(0) a: vec4<f32>,
    @location(1) b: vec4<f32>,
    @location(2) c: vec4<f32>,
    @location(3) d: vec4<f32>,
) -> EdgeOut {
    return edge(vi, k, run.colours.w, point(a, a.y), point(b, b.y), point(c, c.y), point(d, d.y));
}

@fragment
fn fs_edge(i: EdgeOut) -> @location(0) vec4<f32> {
    if outside(i.pos.xy) || i.t <= 0.0 {
        discard;
    }
    return output(unpack(run.colours.z) * i.t);
}
