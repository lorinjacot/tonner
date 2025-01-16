const pi: f32 = 3.14159265359;

struct Attributes {
    @location(1) position: vec3f,
    @location(2) normal: vec3f,
    @location(3) tex_coord_0: vec2f,
    @location(4) tex_coord_1: vec2f,
}

struct Transform {
    model: mat4x4f,
    normal: mat3x3f,
}

struct Camera {
    view_projection: mat4x4f,
    world_position: vec3f,
}

struct FragmentData {
    @builtin(position) position: vec4f,
    @location(0) world_position: vec3f,
    @location(1) world_normal: vec3f,
    @location(2) tex_coord_0: vec2f,
    @location(3) tex_coord_1: vec2f,
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
    metallic_roughness_tex_coord: u32,
    normal_texture_scale: f32,
    normal_tex_coord: u32,
    occlusion_strength: f32,
    occlusion_tex_coord: u32,
    emissive_factor: vec3f,
    emissive_tex_coord: u32,
}

@group(0) @binding(0) var<storage, read> transforms: array<Transform>;

@group(1) @binding(0) var<uniform> camera: Camera;

@group(2) @binding(0) var<uniform> light: Light;

@group(3) @binding(0) var base_color_texture: texture_2d<f32>;
@group(3) @binding(1) var base_color_sampler: sampler;
@group(3) @binding(2) var metallic_roughness_texture: texture_2d<f32>;
@group(3) @binding(3) var metallic_roughness_sampler: sampler;
@group(3) @binding(4) var normal_texture: texture_2d<f32>;
@group(3) @binding(5) var normal_sampler: sampler;
@group(3) @binding(6) var occlusion_texture: texture_2d<f32>;
@group(3) @binding(7) var occlusion_sampler: sampler;
@group(3) @binding(8) var emissive_texture: texture_2d<f32>;
@group(3) @binding(9) var emissive_sampler: sampler;
@group(3) @binding(10) var<uniform> material: Material;

@vertex
fn vs_main(
    @location(0) node_id: u32,
    attributes: Attributes
) -> FragmentData {
    var fragment: FragmentData;
    fragment.position = camera.view_projection * transforms[node_id].model * vec4f(attributes.position, 1.0);
    fragment.tex_coord_0 = attributes.tex_coord_0;
    fragment.tex_coord_1 = attributes.tex_coord_1;
    fragment.world_position = (transforms[node_id].model * vec4f(attributes.position, 1.0)).xyz;
    fragment.world_normal = transforms[node_id].normal * attributes.normal;
    return fragment;
}

@fragment
fn fs_main(
    fragment: FragmentData
) -> @location(0) vec4f {
    let tex_coords = array(
        fragment.tex_coord_0,
        fragment.tex_coord_1
    );

    let base_color = material.base_color_factor * textureSample(
        base_color_texture, 
        base_color_sampler,
        tex_coords[material.base_color_tex_coord],
    );
    let metallic_roughness = textureSample(
        metallic_roughness_texture,
        metallic_roughness_sampler,
        tex_coords[material.metallic_roughness_tex_coord],
    );
    let metalness = material.metallic_factor * metallic_roughness.b;
    let roughness = material.roughness_factor * metallic_roughness.g;

    let occlusion = material.occlusion_strength * textureSample(
        occlusion_texture,
        occlusion_sampler,
        tex_coords[material.occlusion_tex_coord],
    ).r;

    let normal_dir = normalize(fragment.world_normal);
    let view_dir = normalize(camera.world_position - fragment.world_position);

    let f0 = mix(
        vec3f(0.04),
        base_color.rgb,
        metalness,
    );

    var radiance_0: vec3f = vec3f(0.0);
    // for each point light
    {
        // per-light radiance L_i
        let light_dir = normalize(light.world_position - fragment.world_position);
        let halfway_dir = normalize(view_dir + light_dir);
        let distance = length(light.world_position - fragment.world_position);
        let attenuation = 1.0 / (distance * distance);
        let radiance = light.color * attenuation;

        // cook-torrance brdf
        let ndf = distribution_ggx(normal_dir, halfway_dir, roughness);
        let g = geometry_smith(normal_dir, view_dir, light_dir, roughness);
        let f = fresnel_schlick(max(dot(halfway_dir, view_dir), 0.0), f0);

        let k_s = f;
        let k_d = (vec3f(1.0) - k_s) * (1.0 - metalness);

        let numerator = ndf * g * f;
        let denominator = 4.0 * max(dot(normal_dir, view_dir), 0.0) * max(dot(normal_dir, light_dir), 0.0) + 0.0001;
        let specular = numerator / denominator;

        let n_dot_l = max(dot(normal_dir, light_dir), 0.0);
        radiance_0 += (k_d * base_color.rgb / pi + specular) * radiance * n_dot_l;
    }

    let ambient = vec3f(0.03) * base_color.rgb * occlusion;
    let color = ambient + radiance_0;

    return vec4f(color, 1.0);
}

fn distribution_ggx(normal_dir: vec3f, halfway_dir: vec3f, roughness: f32) -> f32 {
    let alpha = roughness*roughness;
    let alpha2 = alpha * alpha;
    let n_dot_h = max(dot(normal_dir, halfway_dir), 0.0);
    let n_dot_h2 = n_dot_h * n_dot_h;

    var denominator: f32 = (n_dot_h2 * (alpha2 - 1.0) + 1.0);
    denominator = pi * denominator * denominator;

    return alpha2 / denominator;
}

fn geometry_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
    let r = (roughness + 1.0);
    let k = (r * r) / 8.0;

    let denominator = n_dot_v * (1.0 - k) + k;

    return n_dot_v / denominator;
}

fn geometry_smith(normal_dir: vec3f, view_dir: vec3f, light_dir: vec3f, roughness: f32) -> f32 {
    let n_dot_v = max(dot(normal_dir, view_dir), 0.0);
    let n_dot_l = max(dot(normal_dir, light_dir), 0.0);
    let ggx_v = geometry_schlick_ggx(n_dot_v, roughness);
    let ggx_l = geometry_schlick_ggx(n_dot_l, roughness);

    return ggx_v * ggx_l;
}

fn fresnel_schlick(cos_theta: f32, f0: vec3f) -> vec3f {
    return f0 + (1.0 - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}