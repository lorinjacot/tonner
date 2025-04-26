struct VertexOutput {
    @builtin(position) position: vec4<f32>,
};

struct Attributes {
    @location(1) position: vec3<f32>,
}

struct NodeUniform {
    model: mat4x4<f32>,
}

struct CameraUniform {
    view_projection: mat4x4<f32>,
}

@group(0) @binding(0) var<storage, read> nodes: array<NodeUniform>;
@group(0) @binding(1) var<uniform> camera: CameraUniform;

@vertex
fn vs_main(
    @location(0) index: u32,
    attributes: Attributes,
) -> VertexOutput {
    let node = nodes[index];

    var result: VertexOutput;
    result.position = camera.view_projection * node.model * vec4(attributes.position, 1.0);
    return result;
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    return vec4(1.0, 0.0, 0.0, 1.0);
}