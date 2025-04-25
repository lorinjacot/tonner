struct VertexOutput {
    @builtin(position) position: vec4<f32>,
};

struct Attributes {
    @location(1) position: vec3<f32>,
}

@vertex
fn vs_main(
    @location(0) index: u32,
    attributes: Attributes,
) -> VertexOutput {
    var result: VertexOutput;
    result.position = vec4(attributes.position, 1.0);
    return result;
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    return vec4(1.0, 0.0, 0.0, 1.0);
}