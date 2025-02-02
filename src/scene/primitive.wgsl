const pi: f32 = 3.14159;
const exposure: f32 = 1.0;

struct Transform {
    @location(0) point_col_x: vec4f,
    @location(1) point_col_y: vec4f,
    @location(2) point_col_z: vec4f,
    @location(3) point_col_w: vec4f,
    @location(4) vector_col_x: vec3f,
    @location(5) vector_col_y: vec3f,
    @location(6) vector_col_z: vec3f,
}

struct Attributes {
    @location(7) position: vec3f,
    @location(8) normal: vec3f,
    @location(9) color_0: vec4f,
    @location(10) tex_coord_0: vec2f,
    @location(11) tex_coord_1: vec2f,
}

struct Camera {
    view_projection: mat4x4f,
    normal_view_projection: mat4x4f,
    world_position: vec3f,
}

struct Fragment {
    @builtin(position) position: vec4f,
    @location(0) world_position: vec3f,
    @location(1) world_normal: vec3f,
    @location(2) tex_coord_0: vec2f,
    @location(3) tex_coord_1: vec2f,
    @location(4) color_0: vec4f,
}

struct Light {
    color: vec3f,
    world_position: vec3f,
}

struct Material {
    base_color_factor: vec4f,
    base_color_tex_coord: u32,
    metallic_factor: f32,
    roughness_factor: f32,
    emissive_factor: vec3f,
    emissive_tex_coord: u32,
}

@group(0) @binding(0) var<uniform> camera: Camera;

@group(1) @binding(0) var<uniform> light: Light;

@group(2) @binding(0) var base_color_texture: texture_2d<f32>;
@group(2) @binding(1) var base_color_sampler: sampler;
@group(2) @binding(2) var emissive_texture: texture_2d<f32>;
@group(2) @binding(3) var emissive_sampler: sampler;
@group(2) @binding(4) var<uniform> material: Material;

@group(3) @binding(0) var irradiance_map_texture: texture_cube<f32>;
@group(3) @binding(1) var irradiance_map_sampler: sampler;

@vertex
fn vs_main(
    transform: Transform,
    attributes: Attributes
) -> Fragment {
    let world_position = mat4x4f(
        transform.point_col_x,
        transform.point_col_y,
        transform.point_col_z,
        transform.point_col_w,
    ) * vec4f(attributes.position, 1.0);

    let world_normal = mat3x3f(
        transform.vector_col_x,
        transform.vector_col_y,
        transform.vector_col_z,
    ) * attributes.normal;

    var fragment: Fragment;
    fragment.position = camera.view_projection * world_position;
    fragment.world_position = world_position.xyz;
    fragment.world_normal = world_normal;
    fragment.tex_coord_0 = attributes.tex_coord_0;
    fragment.tex_coord_1 = attributes.tex_coord_1;
    fragment.color_0 = attributes.color_0;
    return fragment;
}

@fragment
fn fs_main(
    fragment: Fragment
) -> @location(0) vec4f {
    let tex_coords = array(
        fragment.tex_coord_0,
        fragment.tex_coord_1,
    );

    let base_color = material.base_color_factor * fragment.color_0 * textureSample(
        base_color_texture, base_color_sampler, tex_coords[material.base_color_tex_coord]
    );

    let metallic = material.metallic_factor;
    let roughness = material.roughness_factor;
    let alpha = roughness * roughness;
    let alpha_2 = alpha * alpha;

    let occlusion = 1.0;

    let c_diff = base_color.rgb * (1.0 - metallic);
    let f0 = mix(vec3f(0.04), base_color.rgb, metallic);

    let normal = normalize(fragment.world_normal);
    let view_dir = normalize(camera.world_position - fragment.world_position);

    // L_e: emitted radiance
    let emitted_l = material.emissive_factor * textureSample(
        emissive_texture, emissive_sampler, tex_coords[material.emissive_tex_coord]
    ).rgb;

    // ambient lighting (environment map)
    let f = fresnel_roughness(f0, max(dot(normal, view_dir), 0.0), roughness);
    let irradiance = textureSample(irradiance_map_texture, irradiance_map_sampler, normal).rgb;
    let ambient_l = (1.0 - f) * irradiance * base_color.rgb * occlusion;

    // L_r: reflected radiance
    var reflected_l: vec3f = ambient_l;

    // for each light
    {
        let light_dir = normalize(light.world_position - fragment.world_position);
        let halfway_dir = normalize(light_dir + view_dir);

        let distance = length(light.world_position - fragment.world_position);
        let attenuation = 1.0 / (distance * distance);
        let light_radiance = light.color * attenuation;

        let f = fresnel(f0, dot(view_dir, halfway_dir));

        let f_diffuse = (1.0 - f) * diffuse_brdf(c_diff);
        let f_specular = f * specular_brdf(alpha_2, normal, halfway_dir, light_dir, view_dir);
        
        reflected_l += (f_diffuse + f_specular) * light_radiance * max(dot(normal, light_dir), 0.0);
    }

    // L_0: outgoing radiance
    let outgoing_l = emitted_l + reflected_l;

    return vec4f(outgoing_l, base_color.a);
}

/**
 * Lambertian BRDF = c / pi
 */
fn diffuse_brdf(color: vec3f) -> vec3f {
    return color / pi;
}

/**
 * microfacet BRDF = GD / (4 |N.L| |N.V|)
 * G = Smith joint masking-shadowing function
 * D = Trowbridge-Reitz/GGX microfacet distribution
 */
fn specular_brdf(alpha_2: f32, normal: vec3f, halfway_dir: vec3f, light_dir: vec3f, view_dir: vec3f) -> f32 {
    let v = visibility(alpha_2, dot(normal, light_dir), dot(halfway_dir, light_dir))
            * visibility(alpha_2, dot(normal, view_dir), dot(halfway_dir, view_dir));
    let d = distribution(alpha_2, dot(normal, halfway_dir));

    return v * d;
}

/**
 * (Half) visibility function
 * visibility(R) = chi(H.R) / (|N.R| + sqrt(alpha^2 + (1 - alpha^2)(N.R)^2))
 */
fn visibility(alpha_2: f32, n_dot_r: f32, h_dot_r: f32) -> f32 {
    if h_dot_r <= 0.0 {
        return 0.0;
    }

    return 1.0 / (abs(n_dot_r) + sqrt(
        alpha_2 + (1.0 - alpha_2) * n_dot_r * n_dot_r
    ));
}

/**
 * Trowbridge-Reitz/GGX microfacet distribution
 * D = (alpha^2 chi(N.H)) / (pi((N.H)^2(alpha^2 - 1) + 1)^2)
 */
fn distribution(alpha_2: f32, n_dot_h: f32) -> f32 {
    if n_dot_h <= 0.0 {
        return 0.0;
    }

    var denominator: f32 = n_dot_h * n_dot_h * (alpha_2 - 1.0) + 1.0;
    denominator = pi * denominator * denominator;

    return alpha_2 / denominator;
}

/**
 * Schlick Fresnel
 * F = f_0 + (1 - f_0)(1 - |V.H|)^5
 */
fn fresnel(f0: vec3f, v_dot_h: f32) -> vec3f {
    return f0 + (vec3f(1.0) - f0) * pow(1.0 - abs(v_dot_h), 5.0);
}

fn fresnel_roughness(f0: vec3f, v_dot_h: f32, roughness: f32) -> vec3f {
    return f0 + (max(vec3f(1.0 - roughness), f0) - f0) * pow(1.0 - abs(v_dot_h), 5.0);
}