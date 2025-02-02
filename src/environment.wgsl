const pi: f32 = 3.14159;
const inv_atan: vec2f = vec2f(0.1591, 0.3183);

struct Fragment {
    @builtin(position) position: vec4f,
    @location(0) tex_coord: vec3f,
}

struct Camera {
    view_projection: mat4x4f,
    normal_view_projection: mat4x4f,
    world_position: vec3f,
}

@group(0) @binding(0) var<uniform> view_projection: mat4x4f;

@group(0) @binding(0) var<uniform> camera: Camera;

@group(1) @binding(0) var equirectangular_texture: texture_2d<f32>;
@group(1) @binding(1) var equirectangular_sampler: sampler;

@group(1) @binding(0) var environment_map_texture: texture_cube<f32>;
@group(1) @binding(1) var environment_map_sampler: sampler;

@vertex
fn vs_cube_view_projection(
    @location(0) position: vec3f,
) -> Fragment {
    var fragment: Fragment;
    fragment.position = view_projection * vec4f(position, 1.0);
    fragment.tex_coord = vec3f(position.x, position.y, -position.z);
    return fragment;
}

@vertex
fn vs_cube_camera(
    @location(0) position: vec3f,
) -> Fragment {
    var fragment: Fragment;
    fragment.position = (camera.normal_view_projection * vec4f(position, 1.0)).xyww;
    fragment.tex_coord = vec3f(position.x, position.y, -position.z);
    return fragment;
}

@fragment
fn fs_environment_map(
    fragment: Fragment
) -> @location(0) vec4f {
    let normal = normalize(fragment.tex_coord);

    let tex_coord = vec2f(atan2(normal.z, normal.x), asin(normal.y)) * inv_atan + 0.5;

    let color = textureSample(equirectangular_texture, equirectangular_sampler, tex_coord).rgb;

    return vec4f(color, 1.0);
}

@fragment
fn fs_irradiance(
    fragment: Fragment
) -> @location(0) vec4f {
    let normal = normalize(fragment.tex_coord);

    let right = normalize(cross(vec3f(0.0, 1.0, 0.0), normal));
    let up = normalize(cross(normal, right));

    var irradiance: vec3f = vec3f(0.0);

    let sample_delta = 0.025;
    var sample_count: f32 = 0.0;
    for (var phi: f32 = 0.0; phi < 2.0 * pi; phi += sample_delta) {
        for (var theta: f32 = 0.0; theta < 0.5 * pi; theta += sample_delta) {
            // sample direction in tangent space
            let sample_tangent = vec3f(sin(theta) * cos(phi), sin(theta) * sin(phi), cos(theta));
            // sample direction in world space
            let sample_world = sample_tangent.x * right + sample_tangent.y * up + sample_tangent.z * normal;

            irradiance += textureSample(environment_map_texture, environment_map_sampler, sample_world).rgb
                            * cos(theta) * sin(theta);

            sample_count += 1.0;
        }
    }
    irradiance = pi * irradiance / sample_count;

    return vec4f(irradiance, 1.0);
}

@fragment
fn fs_skybox(
    fragment: Fragment
) -> @location(0) vec4f {
    let color = textureSample(environment_map_texture, environment_map_sampler, fragment.tex_coord).rgb;

    return vec4f(color, 1.0);
}