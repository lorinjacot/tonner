struct Attributes {
    @location(0) position: vec3f,
}

struct VertexOutput {
    @builtin(position) position: vec4f,
}

struct Camera {
    view_projection: mat4x4f,
}

@group(0) @binding(0) var<uniform> base_color_factor: vec4f;

@group(1) @binding(0) var<uniform> camera: Camera;

@vertex
fn vs_main(
    attributes: Attributes,
    @builtin(vertex_index) vertex_index: u32,
) -> VertexOutput {
    var out: VertexOutput;
    out.position = camera.view_projection * vec4f(attributes.position, 1.0);
    return out;
}

@fragment
fn fs_main(
    vertex: VertexOutput
) -> @location(0) vec4f {
    return base_color_factor;
}