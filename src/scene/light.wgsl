struct Camera {
    view_projection: mat4x4f,
    view_projection_inverse: mat4x4f,
    world_position: vec3f,
}

@group(0) @binding(0) var<uniform> model: mat4x4f;

@group(1) @binding(0) var<uniform> camera: Camera;

@vertex
fn vs_main(
    @location(0) position: vec3f,
) -> @builtin(position) vec4f {
    return camera.view_projection * model * vec4f(position, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4f {
    return vec4f(1.0);
}