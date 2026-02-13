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

@group(0) @binding(0) var hdr_texture: texture_2d<f32>;
@group(0) @binding(1) var hdr_sampler: sampler;
@group(0) @binding(2) var bloom_texture: texture_2d<f32>;
@group(0) @binding(3) var bloom_sampler: sampler;

@fragment
fn fs_main(
    vertex: VertexOutput
) -> @location(0) vec4<f32> {
    var hdr_color = textureSample(
        hdr_texture,
        hdr_sampler,
        vertex.uv,
    ).rgb;
    let bloom_color = textureSample(
        bloom_texture,
        bloom_sampler,
        vertex.uv,
    ).rgb;
    hdr_color += bloom_color; // additive blending
    // tone mapping
    let result = hdr_color / (hdr_color + vec3(1.0));
    
    return vec4(result, 1.0);
}