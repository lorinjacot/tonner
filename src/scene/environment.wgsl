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

struct AngleRoughness {
    @builtin(position) position: vec4f,
    @location(0) n_dot_v: f32,
    @location(1) roughness: f32,
}

@group(0) @binding(0) var<uniform> view_projection: mat4x4f;

@group(0) @binding(0) var<uniform> camera: Camera;

@group(1) @binding(0) var equirectangular_texture: texture_2d<f32>;
@group(1) @binding(1) var equirectangular_sampler: sampler;

@group(1) @binding(0) var environment_map_texture: texture_cube<f32>;
@group(1) @binding(1) var environment_map_sampler: sampler;

@group(1) @binding(2) var<uniform> roughness: f32;

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

/**
 * https://learnopengl.com/PBR/IBL/Specular-IBL
 */
@fragment
fn fs_prefilter_environment_map(
    in: Fragment
) -> @location(0) vec4f {
    let normal = normalize(in.tex_coord);
    let view_dir = normal;

    var total_weight = 0.0;
    var prefilter_color = vec3f(0.0);
    
    const sample_count: u32 = 4096u;
    for (var i = 0u; i < sample_count; i++) {
        let x_i = hammersley(i, sample_count);
        let halfway_vec = importance_sample_ggx(x_i, normal, roughness);
        let light_dir = -reflect(view_dir, halfway_vec);

        let n_dot_l = max(dot(normal, light_dir), 0.0);
        prefilter_color += textureSample(environment_map_texture, environment_map_sampler, light_dir).rgb * n_dot_l;
        total_weight += n_dot_l;
    }
    prefilter_color = prefilter_color / total_weight;

    return vec4f(prefilter_color, 1.0);
}

@vertex
fn vs_brdf_integration(
   @location(0) position: vec2f,
   @location(1) tex_coord: vec2f,
) -> AngleRoughness {
    var out: AngleRoughness;
    out.position = vec4f(position, 0.0, 1.0);
    out.n_dot_v = tex_coord.x;
    out.roughness = tex_coord.y;
    return out;
}

/**
 * https://learnopengl.com/PBR/IBL/Specular-IBL
 */
@fragment
fn fs_brdf_integration(
    in: AngleRoughness
) -> @location(0) vec2f {
    let view_dir = vec3f(
        sqrt(1.0 - in.n_dot_v * in.n_dot_v),
        0.0,
        in.n_dot_v
    );
    
    var a = 0.0;
    var b = 0.0;

    let normal = vec3f(0.0, 0.0, 1.0);

    const sample_count = 1024u;
    for (var i = 0u; i < sample_count; i++) {
        let x_i = hammersley(i, sample_count);
        let halfway_vec = importance_sample_ggx(x_i, normal, in.roughness);
        let light_dir = normalize(-reflect(view_dir, halfway_vec));

        let n_dot_l = max(light_dir.z, 0.0);
        let n_dot_h = max(halfway_vec.z, 0.0);
        let v_dot_h = max(dot(view_dir, halfway_vec), 0.0);

        if (n_dot_l > 0.0) {
            let g = geometry_smith(normal, view_dir, light_dir, in.roughness);
            let g_vis = (g * v_dot_h) / (n_dot_h * in.n_dot_v);
            let fc = pow(1.0 - v_dot_h, 5.0);

            a += (1.0 - fc) * g_vis;
            b += fc * g_vis;
        }
    }
    a /= f32(sample_count);
    b /= f32(sample_count);
    return vec2f(a, b);
}

fn geometry_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32
{
    let a = roughness;
    let k = (a * a) / 2.0;

    let nom = n_dot_v;
    let denom = n_dot_v * (1.0 - k) + k;

    return nom / denom;
}

fn geometry_smith(normal: vec3f, view_dir: vec3f, light_dir: vec3f, roughness: f32) -> f32
{
    let n_dot_v = max(dot(normal, view_dir), 0.0);
    let n_dot_l = max(dot(normal, light_dir), 0.0);
    let ggx2 = geometry_schlick_ggx(n_dot_v, roughness);
    let ggx1 = geometry_schlick_ggx(n_dot_l, roughness);

    return ggx1 * ggx2;
}  

fn importance_sample_ggx(x_i: vec2f, normal: vec3f, roughness: f32) -> vec3f {
    let a = roughness * roughness;

    let phi = 2.0 * pi * x_i.x;
    let cos_theta = sqrt((1.0 - x_i.y) / (1.0 + (a*a - 1.0) * x_i.y));
    let sin_theta = sqrt(1.0 - cos_theta * cos_theta);

    // from spherical coordinates to cartesian coordinates
    let halfway_vec = vec3f(
        cos(phi) * sin_theta,
        sin(phi) * sin_theta,
        cos_theta,
    );

    // from tangent-space vector to world-space sample vector
    var up = vec3f(0.0, 0.0, 1.0);
    if (abs(normal.z) >= 0.999) {
        up = vec3f(1.0, 0.0, 0.0);
    };
    let tangent = normalize(cross(up, normal));
    let bitangent = cross(normal, tangent);

    let sample_vec = mat3x3f(tangent, bitangent, normal) * halfway_vec;
    return normalize(sample_vec);
}

/*
 * Based on Van der Corput sequence
 */
fn radical_inverse(i: u32) -> f32 {
    var bits = i;
    bits = (bits << 16u) | (bits >> 16u);
    bits = ((bits & 0x55555555u) << 1u) | ((bits & 0xAAAAAAAAu) >> 1u);
    bits = ((bits & 0x33333333u) << 2u) | ((bits & 0xCCCCCCCCu) >> 2u);
    bits = ((bits & 0x0F0F0F0Fu) << 4u) | ((bits & 0xF0F0F0F0u) >> 4u);
    bits = ((bits & 0x00FF00FFu) << 8u) | ((bits & 0xFF00FF00u) >> 8u);
    return f32(bits) * 2.3283064365386963e-10; // / 0x100000000
}

/**
 * Generate sample i of Hammersley Sequence of n total samples.
 */
fn hammersley(i: u32, n: u32) -> vec2f {
    return vec2f(f32(i)/f32(n), radical_inverse(i));
}
