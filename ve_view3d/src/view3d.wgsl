// ve_view3d: faces lit by a headlight, edges as screen-space quads of a fixed width (V3D-1).
// Rendered offscreen into an Rgba8Unorm target with depth and 4x MSAA; egui shows the resolved texture, whose
// bytes it takes as gamma-encoded. Shading is in linear light, encoded to sRGB at the end.

struct Uniforms {
    view_proj: mat4x4<f32>,
    // xyz: the eye; w: 1 for an orthographic projection.
    eye: vec4<f32>,
    // xyz: the view direction.
    forward: vec4<f32>,
    // xy: the target size in pixels; z: pixels per point.
    viewport: vec4<f32>,
    // x: the hovered face key, y: the hovered edge key (0: none).
    hover: vec4<u32>,
    // Up to 16 selected keys, faces and edges alike (faces count from 1, edges from 0x80000001).
    selected: array<vec4<u32>, 4>,
    hover_color: vec4<f32>,
    select_color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

fn linear_from_srgb(c: vec3<f32>) -> vec3<f32> {
    let cutoff = c < vec3<f32>(0.04045);
    let lower = c / 12.92;
    let higher = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(higher, lower, cutoff);
}

fn srgb_from_linear(c: vec3<f32>) -> vec3<f32> {
    let cutoff = c < vec3<f32>(0.0031308);
    let lower = c * 12.92;
    let higher = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(higher, lower, cutoff);
}

fn is_selected(key: u32) -> bool {
    if key == 0u {
        return false;
    }
    for (var i = 0; i < 4; i++) {
        if any(u.selected[i] == vec4<u32>(key)) {
            return true;
        }
    }
    return false;
}

// Mix the hover or selection colour in (both given in linear RGB).
fn highlight(base: vec3<f32>, key: u32, hovered: u32, amount: f32) -> vec3<f32> {
    var c = base;
    if is_selected(key) {
        c = mix(c, u.select_color.rgb, amount);
    }
    if key != 0u && key == hovered {
        c = mix(c, u.hover_color.rgb, amount * 0.8);
    }
    return c;
}

struct MeshOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) @interpolate(flat) key: u32,
};

@vertex
fn vs_mesh(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) key: u32,
) -> MeshOut {
    var out: MeshOut;
    out.pos = u.view_proj * vec4<f32>(position, 1.0);
    out.world = position;
    out.normal = normal;
    out.color = color;
    out.key = key;
    return out;
}

@fragment
fn fs_mesh(in: MeshOut) -> @location(0) vec4<f32> {
    var to_eye = -u.forward.xyz;
    if u.eye.w < 0.5 {
        to_eye = normalize(u.eye.xyz - in.world);
    }
    var n = normalize(in.normal);
    // Two-sided: a face seen from behind is lit like its front.
    if dot(n, to_eye) < 0.0 {
        n = -n;
    }
    // A headlight (a face toward the viewer is bright, one seen edge-on dark) with a little specular, and a soft
    // light from above (+Z) so faces at the same angle to the viewer (the iso view's three) still differ.
    let lambert = max(dot(n, to_eye), 0.0);
    let spec = pow(lambert, 60.0) * 0.04;
    let sky = 0.5 + 0.5 * n.z;
    let base = highlight(linear_from_srgb(in.color.rgb), in.key, u.hover.x, 0.55);
    let lit = base * (0.18 + 0.62 * lambert + 0.25 * sky) + vec3<f32>(spec);
    return vec4<f32>(srgb_from_linear(clamp(lit, vec3<f32>(0.0), vec3<f32>(1.0))), 1.0);
}

struct EdgeOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_edge(
    @builtin(vertex_index) vi: u32,
    @location(0) a_in: vec3<f32>,
    @location(1) b_in: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) key: u32,
    @location(4) width: f32,
) -> EdgeOut {
    var out: EdgeOut;
    var a = u.view_proj * vec4<f32>(a_in, 1.0);
    var b = u.view_proj * vec4<f32>(b_in, 1.0);
    // Clip against the near plane (z = 0 in wgpu's clip space) so a segment through the eye still draws.
    if a.z < 0.0 && b.z < 0.0 {
        out.pos = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        out.color = vec4<f32>(0.0);
        return out;
    }
    if a.z < 0.0 {
        a = mix(a, b, a.z / (a.z - b.z));
    } else if b.z < 0.0 {
        b = mix(b, a, b.z / (b.z - a.z));
    }
    // Corners of the quad: x picks the end, y the side.
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );
    let corner = corners[vi % 6u];
    let half_px = u.viewport.xy * 0.5;
    let sa = a.xy / a.w * half_px;
    let sb = b.xy / b.w * half_px;
    var dir = sb - sa;
    let len = length(dir);
    if len < 1e-4 {
        dir = vec2<f32>(1.0, 0.0);
    } else {
        dir = dir / len;
    }
    let side = vec2<f32>(-dir.y, dir.x);
    var w = width * u.viewport.z;
    var c = linear_from_srgb(color.rgb);
    let hovered = key == u.hover.y;
    let selected = is_selected(key);
    if hovered || selected {
        w = w * 2.0;
        c = highlight(c, key, u.hover.y, 0.85);
    }
    // Half the width out to the side, and half past each end so polylines join without gaps.
    let along = select(-1.0, 1.0, corner.x > 0.5);
    let offset_px = side * corner.y * w * 0.5 + dir * along * w * 0.5;
    var clip = select(a, b, corner.x > 0.5);
    clip = vec4<f32>(clip.xy + offset_px / half_px * clip.w, clip.z, clip.w);
    // Pull toward the eye so an edge on a face wins the depth test against it.
    clip.z = max(clip.z - 4e-4 * clip.w, 0.0);
    out.pos = clip;
    out.color = vec4<f32>(c, 1.0);
    return out;
}

@fragment
fn fs_edge(in: EdgeOut) -> @location(0) vec4<f32> {
    return vec4<f32>(srgb_from_linear(in.color.rgb), in.color.a);
}
