struct Fragment {
    @builtin(position) position: vec4f,
    @location(0) clip_position: vec3f,
}

struct Camera {
    view_projection: mat4x4f,
    view_projection_inverse: mat4x4f,
    world_position: vec3f,
}

@group(0) @binding(0) var skybox_texture: texture_cube<f32>;
@group(0) @binding(1) var skybox_sampler: sampler;

@group(1) @binding(0) var<uniform> view_proj_inverse: mat4x4f;

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
) -> Fragment {
    let positions = array(
        vec2f(-1.0,  3.0),
        vec2f(-1.0, -1.0),
        vec2f( 3.0, -1.0),
    );
    var fragment: Fragment;
    let position = vec3f(positions[vertex_index], 1.0);
    fragment.position = vec4f(position, 1.0);
    fragment.clip_position = position;
    return fragment;
}

@fragment
fn fs_main(
    fragment: Fragment
) -> @location(0) vec4f {
    let normal = normalize((view_proj_inverse * vec4f(fragment.clip_position, 1.0)).xyz);
    let color = textureSample(skybox_texture, skybox_sampler, normal).rgb;

    return vec4f(color, 1.0);
}