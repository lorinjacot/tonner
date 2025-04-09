override has_tangent: bool;
override has_tex_coord_0: bool;
override has_tex_coord_1: bool;
override has_color_0: bool;

override has_base_color_texture: bool;
override has_metallic_roughness_texture: bool;
override has_normal_texture: bool;
override has_occlusion_texture: bool;
override has_emissive_texture: bool;

const pi: f32 = 3.14159;
const tex_coord_count: u32 = 2u;
const prefiltered_environment_mipmap_count: u32 = 4u;

struct Transform {
    @location(0) point_col_x: vec4<f32>,
    @location(1) point_col_y: vec4<f32>,
    @location(2) point_col_z: vec4<f32>,
    @location(3) point_col_w: vec4<f32>,
    @location(4) vector_col_x: vec3<f32>,
    @location(5) vector_col_y: vec3<f32>,
    @location(6) vector_col_z: vec3<f32>,
}

struct Attributes {
    @location(7) position: vec3<f32>,
    @location(8) normal: vec3<f32>,
    @location(9) tangent: vec4<f32>,
    @location(10) tex_coord_0: vec2<f32>,
    @location(11) tex_coord_1: vec2<f32>,
    @location(12) color_0: vec4<f32>,
}

struct Camera {
    view_projection: mat4x4<f32>,
    normal_view_projection: mat4x4<f32>,
    world_position: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec3<f32>,
    @location(3) bitangent: vec3<f32>,
    @location(4) tex_coord_0: vec2<f32>,
    @location(5) tex_coord_1: vec2<f32>,
    @location(6) color_0: vec4<f32>,
}

struct Light {
    color: vec3<f32>,
    world_position: vec3<f32>,
}

struct Material {
    base_color_factor: vec4<f32>,
    base_color_tex_coord: u32,
    metallic_factor: f32,
    roughness_factor: f32,
    metallic_roughness_tex_coord: u32,
    normal_scale: f32,
    normal_tex_coord: u32,
    occlusion_strength: f32,
    occlusion_tex_coord: u32,
    emissive_factor: vec3<f32>,
    emissive_tex_coord: u32,
}

@group(0) @binding(0) var<uniform> camera: Camera;

// @group(1) @binding(0) var<uniform> light: Light;

@group(1) @binding(0) var base_color_texture: texture_2d<f32>;
@group(1) @binding(1) var base_color_sampler: sampler;
@group(1) @binding(2) var metallic_roughness_texture: texture_2d<f32>;
@group(1) @binding(3) var metallic_roughness_sampler: sampler;
@group(1) @binding(4) var normal_texture: texture_2d<f32>;
@group(1) @binding(5) var normal_sampler: sampler;
@group(1) @binding(6) var occlusion_texture: texture_2d<f32>;
@group(1) @binding(7) var occlusion_sampler: sampler;
@group(1) @binding(8) var emissive_texture: texture_2d<f32>;
@group(1) @binding(9) var emissive_sampler: sampler;
@group(1) @binding(10) var<uniform> material: Material;

// @group(3) @binding(0) var irradiance_map_texture: texture_cube<f32>;
// @group(3) @binding(1) var irradiance_map_sampler: sampler;
// @group(3) @binding(2) var prefiltered_environment_map_texture: texture_cube<f32>;
// @group(3) @binding(3) var prefiltered_environment_map_sampler: sampler;
// @group(3) @binding(4) var brdf_integration_map_texture: texture_2d<f32>;
// @group(3) @binding(5) var brdf_integration_map_sampler: sampler;

@vertex
fn vs_main(
    transform: Transform,
    attributes: Attributes
) -> VertexOutput {
    let world_position = mat4x4(
        transform.point_col_x,
        transform.point_col_y,
        transform.point_col_z,
        transform.point_col_w,
    ) * vec4f(attributes.position, 1.0);

    let vector_transform = mat3x3(
        transform.vector_col_x,
        transform.vector_col_y,
        transform.vector_col_z,
    );

    var result: VertexOutput;
    result.position = camera.view_projection * world_position;
    result.world_position = world_position.xyz;
    result.normal = vector_transform * attributes.normal;
    if has_tangent {
        result.tangent = vector_transform * attributes.tangent.xyz;
        result.bitangent = cross(result.normal, result.tangent) * attributes.tangent.w;
    }
    if has_tex_coord_0 {
        result.tex_coord_0 = attributes.tex_coord_0;
    }
    if has_tex_coord_1 {
        result.tex_coord_1 = attributes.tex_coord_1;
    }
    if has_color_0 {
        result.color_0 = attributes.color_0;
    }
    return result;
}

@fragment
fn fs_main(
    in: VertexOutput
) -> @location(0) vec4<f32> {
    var tex_coords = array(
        in.tex_coord_0,
        in.tex_coord_1,
    );

    var base_color = material.base_color_factor * in.color_0;
    if has_base_color_texture {
        base_color *= textureSample(
            base_color_texture,
            base_color_sampler,
            tex_coords[material.base_color_tex_coord]
        );
    }

    var roughness = material.roughness_factor;
    var metalness = material.metallic_factor;
    if has_metallic_roughness_texture {
        let metallic_roughness = textureSample(
            metallic_roughness_texture,
            metallic_roughness_sampler,
            tex_coords[material.metallic_roughness_tex_coord],
        );
        var roughness = roughness * metallic_roughness.g;
        var metalness = metallic_roughness * metallic_roughness.b;
    }

    var normal: vec3f;
    if has_normal_texture {
        // use tangent space normal texture
        normal = textureSample(
            normal_texture,
            normal_sampler,
            tex_coords[material.normal_tex_coord],
        ).rgb * 2.0 - 1.0;

        let transform = mat3x3(in.tangent, in.bitangent, in.normal);
        normal = (
            normalize(transform * normal)
            * vec3f(material.normal_scale, material.normal_scale, 1.0)
        );
    } else {
        normal = normalize(in.normal);
    }

    var occlusion = 1.0;
    if has_normal_texture {
        occlusion += material.occlusion_strength * (textureSample(
            occlusion_texture,
            occlusion_sampler,
            tex_coords[material.occlusion_tex_coord],
        ).r - 1.0);
    }

    var emissive = material.emissive_factor;
    if has_emissive_texture {
        emissive = emissive * textureSample(
            emissive_texture,
            emissive_sampler,
            tex_coords[material.emissive_tex_coord]
        ).rgb;
    }

    let c_diff = base_color.rgb * (1.0 - metalness);
    let f0 = mix(vec3f(0.04), base_color.rgb, metalness);

    let alpha = roughness * roughness;
    let alpha_2 = alpha * alpha;
    
    let view_dir = normalize(camera.world_position - in.world_position);

    // image based lighting (IBL)
    // let n_dot_v = max(dot(normal, view_dir), 0.0);
    // let f = fresnel_roughness(f0, n_dot_v, roughness);
    
    // let irradiance = textureSample(irradiance_map_texture, irradiance_map_sampler, normal).rgb;
    // let diffuse = (1.0 - f) * irradiance * base_color.rgb;

    // let reflection_vec = reflect(-view_dir, normal);
    // let prefiltered_color = textureSampleLevel(
    //     prefiltered_environment_map_texture,
    //     prefiltered_environment_map_sampler,
    //     reflection_vec,
    //     roughness * f32(prefiltered_environment_mipmap_count),
    // ).rgb;

    // let environment_brdf = textureSample(
    //     brdf_integration_map_texture,
    //     brdf_integration_map_sampler,
    //     vec2f(n_dot_v, roughness),
    // ).rg;

    // let specular = prefiltered_color * (f * environment_brdf.x + environment_brdf.y);

    // let ambient = (diffuse + specular) * occlusion;
    let ambient = vec3(1.0);

    // L_r: reflected radiance
    var reflected_l = ambient;

    // for each light
    // {
    //     let light_dir = normalize(light.world_position - in.world_position);
    //     let halfway_dir = normalize(light_dir + view_dir);

    //     let distance = length(light.world_position - in.world_position);
    //     let attenuation = 1.0 / (distance * distance);
    //     let light_radiance = light.color * attenuation;

    //     let f = fresnel(f0, dot(view_dir, halfway_dir));

    //     let f_diffuse = (1.0 - f) * diffuse_brdf(c_diff);
    //     let f_specular = f * specular_brdf(alpha_2, normal, halfway_dir, light_dir, view_dir);
        
    //     reflected_l += (f_diffuse + f_specular) * light_radiance * max(dot(normal, light_dir), 0.0);
    // }

    // L_0: outgoing radiance
    let outgoing_l = emissive + reflected_l;


    return vec4f(outgoing_l, base_color.a);
    // return vec4f(pow(outgoing_l, vec3f(1.0/2.2)), base_color.a);
    // return vec4f(pow(base_color.rgb, vec3f(2.2)), base_color.a);
}

///**
// * Lambertian BRDF = c / pi
// */
fn diffuse_brdf(color: vec3f) -> vec3f {
    return color / pi;
}

///**
// * microfacet BRDF = GD / (4 |N.L| |N.V|)
// * G = Smith joint masking-shadowing function
// * D = Trowbridge-Reitz/GGX microfacet distribution
// */
fn specular_brdf(alpha_2: f32, normal: vec3f, halfway_dir: vec3f, light_dir: vec3f, view_dir: vec3f) -> f32 {
    let v = visibility(alpha_2, dot(normal, light_dir), dot(halfway_dir, light_dir))
            * visibility(alpha_2, dot(normal, view_dir), dot(halfway_dir, view_dir));
    let d = distribution(alpha_2, dot(normal, halfway_dir));

    return v * d;
}

///**
// * (Half) visibility function
// * visibility(R) = chi(H.R) / (|N.R| + sqrt(alpha^2 + (1 - alpha^2)(N.R)^2))
// */
fn visibility(alpha_2: f32, n_dot_r: f32, h_dot_r: f32) -> f32 {
    if h_dot_r <= 0.0 {
        return 0.0;
    }

    return 1.0 / (abs(n_dot_r) + sqrt(
        alpha_2 + (1.0 - alpha_2) * n_dot_r * n_dot_r
    ));
}

///**
// * Trowbridge-Reitz/GGX microfacet distribution
// * D = (alpha^2 chi(N.H)) / (pi((N.H)^2(alpha^2 - 1) + 1)^2)
// */
fn distribution(alpha_2: f32, n_dot_h: f32) -> f32 {
    if n_dot_h <= 0.0 {
        return 0.0;
    }

    var denominator: f32 = n_dot_h * n_dot_h * (alpha_2 - 1.0) + 1.0;
    denominator = pi * denominator * denominator;

    return alpha_2 / denominator;
}

///**
// * Schlick Fresnel
// * F = f_0 + (1 - f_0)(1 - |V.H|)^5
// */
fn fresnel(f0: vec3f, v_dot_h: f32) -> vec3f {
    return f0 + (vec3f(1.0) - f0) * pow(1.0 - abs(v_dot_h), 5.0);
}

fn fresnel_roughness(f0: vec3f, v_dot_h: f32, roughness: f32) -> vec3f {
    return f0 + (max(vec3f(1.0 - roughness), f0) - f0) * pow(1.0 - abs(v_dot_h), 5.0);
}