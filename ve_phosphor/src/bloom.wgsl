// The bloom (after common.wgsl): a separable Gaussian blur of the light above the threshold, horizontal then
// vertical.

@group(0) @binding(1) var src: texture_2d<f32>;
@group(0) @binding(2) var dst: texture_storage_2d<rgba16float, write>;

fn blur(at: vec2<i32>, dir: vec2<i32>) -> vec4<f32> {
    let size = vec2<i32>(textureDimensions(src));
    let r = i32(clamp(p.look.w, 1.0, 32.0));
    let sigma = max(p.look.w * 0.5, 0.5);
    var sum = vec3<f32>(0.0);
    var wsum = 0.0;
    for (var k = -r; k <= r; k++) {
        let q = clamp(at + dir * k, vec2<i32>(0), size - vec2<i32>(1));
        let w = exp(-0.5 * f32(k * k) / (sigma * sigma));
        sum += textureLoad(src, q, 0).rgb * w;
        wsum += w;
    }
    return vec4<f32>(sum / wsum, 1.0);
}

@compute @workgroup_size(8, 8, 1)
fn blur_h(@builtin(global_invocation_id) gid: vec3<u32>) {
    let size = textureDimensions(src);
    if gid.x >= size.x || gid.y >= size.y {
        return;
    }
    let at = vec2<i32>(i32(gid.x), i32(gid.y));
    textureStore(dst, at, blur(at, vec2<i32>(1, 0)));
}

@compute @workgroup_size(8, 8, 1)
fn blur_v(@builtin(global_invocation_id) gid: vec3<u32>) {
    let size = textureDimensions(src);
    if gid.x >= size.x || gid.y >= size.y {
        return;
    }
    let at = vec2<i32>(i32(gid.x), i32(gid.y));
    textureStore(dst, at, blur(at, vec2<i32>(0, 1)));
}
