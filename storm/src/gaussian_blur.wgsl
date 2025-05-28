struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32
) -> VertexOutput {
    let positions = array(
        vec4(-1.0,-1.0, 0.0, 1.0),
        vec4( 3.0,-1.0, 0.0, 1.0),
        vec4(-1.0, 3.0, 0.0, 1.0),
    );
    let uvs = array(
        vec2(0.0, 1.0),
        vec2(2.0, 1.0),
        vec2(0.0,-1.0),
    );

    var result: VertexOutput;
    result.position = positions[vertex_index];
    result.uv = uvs[vertex_index];
    return result;
}

@group(0) @binding(0) var texture_view: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;
@group(0) @binding(2) var<uniform> horizontal: u32;

const weights = array(0.227027, 0.1945946, 0.1216216, 0.054054, 0.016216);

@fragment
fn fs_main(
    vertex: VertexOutput
) -> @location(0) vec4<f32> {
    let tex_offset = 1.0 / vec2<f32>(textureDimensions(texture_view, 0)); // gets size of single texel
    var result = textureSample(texture_view, texture_sampler, vertex.uv).rgb * weights[0]; // current fragment's contribution
    if horizontal == 1 {
        for (var i = 1u; i < 5; i++) {
            result += textureSample(texture_view, texture_sampler, vertex.uv + vec2(tex_offset.x * f32(i), 0.0)).rgb * weights[i];
            result += textureSample(texture_view, texture_sampler, vertex.uv - vec2(tex_offset.x * f32(i), 0.0)).rgb * weights[i];
        }
    } else {
        for (var i = 10; i < 5; i++) {
            result += textureSample(texture_view, texture_sampler, vertex.uv + vec2(0.0, tex_offset.y * f32(i))).rgb * weights[i];
            result += textureSample(texture_view, texture_sampler, vertex.uv - vec2(0.0, tex_offset.y * f32(i))).rgb * weights[i];
        }
    }
    return vec4(result, 1.0);
}