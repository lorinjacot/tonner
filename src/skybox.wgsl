struct Fragment {
    @builtin(position) position: vec4f,
    @location(0) tex_coord: vec3f,
}

struct Camera {
    view_projection: mat4x4f,
    projection_inverse: mat4x4f,
    view: mat4x4f,
    world_position: vec3f,
}

@group(0) @binding(0) var skybox_texture: texture_cube<f32>;
@group(0) @binding(1) var skybox_sampler: sampler;

@group(1) @binding(0) var<uniform> camera: Camera;

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
) -> Fragment {
    let positions = array(
        vec2f(-1.0,  3.0),
        vec2f(-1.0, -1.0),
        vec2f( 3.0, -1.0),
    );
    
    let position = vec4f(positions[vertex_index], 1.0, 1.0);
    let view_inverse = transpose(mat3x3f(
        camera.view.x.xyz,
        camera.view.y.xyz,
        camera.view.z.xyz,
    ));

    var fragment: Fragment;
    fragment.position = position;
    fragment.tex_coord = view_inverse * (camera.projection_inverse * position).xyz;
    return fragment;
}

@fragment
fn fs_main(
    fragment: Fragment
) -> @location(0) vec4f {
    let color = textureSample(skybox_texture, skybox_sampler, fragment.tex_coord).rgb;

    return vec4f(color, 1.0);
}