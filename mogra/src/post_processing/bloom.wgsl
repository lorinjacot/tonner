struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct Parameters {
    /// distance to closest neigbour to sample for downsampling
    delta: vec2<f32>,

    /// distance to closest neigbour to sample for upsampling
    radius: f32,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let positions = array(
            vec4(-1.0, -1.0, 0.0, 1.0),
            vec4(3.0, -1.0, 0.0, 1.0),
            vec4(-1.0, 3.0, 0.0, 1.0),
        );
    let uvs = array(vec2(0.0, 1.0), vec2(2.0, 1.0), vec2(0.0, -1.0));

    var result: VertexOutput;
    result.position = positions[vertex_index];
    result.uv = uvs[vertex_index];
    return result;
}

@group(0) @binding(0)
var t: texture_2d<f32>;

@group(0) @binding(1)
var s: sampler;

@group(0) @binding(2)
var<uniform> parameters: Parameters;

@fragment
fn fs_downsample(vertex: VertexOutput) -> @location(0) vec4<f32> {
    let u = vertex.uv.x;
    let v = vertex.uv.y;

    let du = parameters.delta.x;
    let dv = parameters.delta.y;

    // Take 13 samples around current texel e:
    // a - d - g
    // - j - l -
    // b - e - h
    // - k - m -
    // c - f - i
    let a = textureSample(t, s, vec2(u - 2 * du, v - 2 * dv));
    let b = textureSample(t, s, vec2(u - 2 * du, v));
    let c = textureSample(t, s, vec2(u - 2 * du, v + 2 * dv));

    let d = textureSample(t, s, vec2(u, v + 2 * dv));
    let e = textureSample(t, s, vec2(u, v));
    let f = textureSample(t, s, vec2(u, v - 2 * dv));

    let g = textureSample(t, s, vec2(u + 2 * du, v - 2 * dv));
    let h = textureSample(t, s, vec2(u + 2 * du, v));
    let i = textureSample(t, s, vec2(u + 2 * du, v + 2 * dv));

    let j = textureSample(t, s, vec2(u - du, v - dv));
    let k = textureSample(t, s, vec2(u - du, v + dv));
    let l = textureSample(t, s, vec2(u + du, v - dv));
    let m = textureSample(t, s, vec2(u + du, v + dv));

    var downsample = 0.125 * e;
    downsample += 0.03125 * (a + c + g + i);
    downsample += 0.0625 * (b + d + f + h);
    downsample += 0.125 * (j + k + l + m);
    return downsample;
}

@fragment
fn fs_upsample(vertex: VertexOutput) -> @location(0) vec4<f32> {
    let u = vertex.uv.x;
    let v = vertex.uv.y;

    let du = parameters.delta.x;
    let dv = parameters.delta.y;

    // Take 9 samples around current texel:
    // a - d - g
    // b - e - h
    // c - f - i
    let a = textureSample(t, s, vec2(u - du, v - dv));
    let b = textureSample(t, s, vec2(u - du, v));
    let c = textureSample(t, s, vec2(u - du, v + dv));

    let d = textureSample(t, s, vec2(u, v - dv));
    let e = textureSample(t, s, vec2(u, v));
    let f = textureSample(t, s, vec2(u, v + dv));

    let g = textureSample(t, s, vec2(u + du, v - dv));
    let h = textureSample(t, s, vec2(u + du, v));
    let i = textureSample(t, s, vec2(u + du, v + dv));

    var upsample = 0.25 * e;
    upsample += 0.125 * (b + d + f + h);
    upsample += 0.0625 * (a + c + g + i);
    return upsample;
}
