struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec3<f32>,
}

struct CameraUniform {
    view_projection: mat4x4<f32>,
    view: mat4x4<f32>,
    projection_inverse: mat4x4<f32>,
    position: vec3<f32>,
}

@group(0) @binding(1) var<uniform> camera: CameraUniform;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // hacky way to draw a large triangle
    let tmp1 = i32(vertex_index) / 2;
    let tmp2 = i32(vertex_index) & 1;
    let pos = vec4<f32>(
        f32(tmp1) * 4.0 - 1.0,
        f32(tmp2) * 4.0 - 1.0,
        1.0,
        1.0
    );

    let view_inverse = transpose(mat3x3(
        camera.view[0].xyz,
        camera.view[1].xyz,
        camera.view[2].xyz,
    ));
    let unprojected = camera.projection_inverse * pos;

    var result: VertexOutput;
    result.uv = view_inverse * unprojected.xyz;
    result.position = pos;
    return result;
}

@group(1) @binding(0) var skybox_texture: texture_cube<f32>;
@group(1) @binding(1) var skybox_sampler: sampler;

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(skybox_texture, skybox_sampler, vertex.uv);
}
