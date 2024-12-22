struct Node {
    model: mat4x4f,
}

struct Attributes {
    @location(1) position: vec3f,
}

struct Camera {
    view_projection: mat4x4f,
}

struct VertexOutput {
    @builtin(position) position: vec4f,
}

@group(0) @binding(0) var<storage, read> nodes: array<Node>;

@group(1) @binding(0) var<uniform> camera: Camera;

@vertex
fn vs_main(
    @location(0) node_id: u32,
    attributes: Attributes
) -> VertexOutput {
    var out: VertexOutput;
    out.position = camera.view_projection * nodes[node_id].model * vec4f(attributes.position, 1.0);
    return out;
}

@fragment
fn fs_main(
    in: VertexOutput
) -> @location(0) vec4f {
    return vec4f(1.0, 0.0, 0.0, 1.0);
}