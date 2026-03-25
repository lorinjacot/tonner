override geometry_flags: u32;
override material_flags: u32;
override alpha_mode: u32;

override max_prefilter_map_mip: f32;

const pi = 3.14159265359;
const max_weight_count = 8;

const geometry_position_flag: u32    = 1 << 0;
const geometry_normal_flag: u32      = 1 << 1;
const geometry_tangent_flag: u32     = 1 << 2;
const geometry_tex_coord_0_flag: u32 = 1 << 3;
const geometry_tex_coord_1_flag: u32 = 1 << 4;
const geometry_color_0_flag: u32     = 1 << 5;
const geometry_joints_0_flag: u32    = 1 << 6;
const geometry_weights_0_flag: u32   = 1 << 7;

const material_base_color_texture_flag: u32         = 1 << 0;
const material_metallic_roughness_texture_flag: u32 = 1 << 1;
const material_normal_texture_flag: u32             = 1 << 2;
const material_occlusion_texture_flag: u32          = 1 << 3;
const material_emissive_texture_flag: u32           = 1 << 4;

struct CameraUniform {
    view_projection: mat4x4<f32>,
    view: mat4x4<f32>,
    projection_inverse: mat4x4<f32>,
    position: vec3<f32>,
}

struct PointLightStorage {
    count: u32,
    data: array<PointLight>,
}

struct PointLight {
    position: vec3<f32>,
    color: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) world_tangent: vec4<f32>,
    @location(3) tex_coord_0: vec2<f32>,
    @location(4) tex_coord_1: vec2<f32>,
    @location(5) color_0: vec4<f32>,
}

@group(0) @binding(1) var<storage, read> joint_matrices: array<mat4x4<f32>>;
@group(0) @binding(2) var<uniform> camera: CameraUniform;
@group(0) @binding(3) var<storage, read> point_lights: PointLightStorage;
@group(0) @binding(4) var irradiance_map_texture: texture_cube<f32>;
@group(0) @binding(5) var irradiance_map_sampler: sampler;
@group(0) @binding(6) var prefilter_map_texture: texture_cube<f32>;
@group(0) @binding(7) var prefilter_map_sampler: sampler;
@group(0) @binding(8) var brdf_lut_texture: texture_2d<f32>;
@group(0) @binding(9) var brdf_lut_sampler: sampler;

struct GeometryStorage {
    vertex_count: u32,
    target_count: u32,
    attributes: array<Attribute>,
}

struct Attribute {
    position: vec3<f32>,
    normal: vec3<f32>,
    tangent: vec4<f32>,
    tex_coord_0: vec2<f32>,
    tex_coord_1: vec2<f32>,
    color_0: vec4<f32>,
    joints_0: vec4<u32>,
    weights_0: vec4<f32>,
}

struct MaterialUniform {
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
    alpha_cutoff: f32,
}

@group(1) @binding(0) var<storage, read> geometry: GeometryStorage;
@group(1) @binding(1) var<uniform> material_uniform: MaterialUniform;
@group(1) @binding(2) var base_color_texture: texture_2d<f32>;
@group(1) @binding(3) var base_color_sampler: sampler;
@group(1) @binding(4) var metallic_roughness_texture: texture_2d<f32>;
@group(1) @binding(5) var metallic_roughness_sampler: sampler;
@group(1) @binding(6) var normal_texture: texture_2d<f32>;
@group(1) @binding(7) var normal_sampler: sampler;
@group(1) @binding(8) var occlusion_texture: texture_2d<f32>;
@group(1) @binding(9) var occlusion_sampler: sampler;
@group(1) @binding(10) var emissive_texture: texture_2d<f32>;
@group(1) @binding(11) var emissive_sampler: sampler;

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) model_col_0: vec4<f32>,
    @location(1) model_col_1: vec4<f32>,
    @location(2) model_col_2: vec4<f32>,
    @location(3) model_col_3: vec4<f32>,
    @location(4) weights_0: vec4<f32>,
    @location(5) weights_1: vec4<f32>,
    @location(6) joint_offset: u32,
) -> VertexOutput {
    var weights = array(
        weights_0.x,
        weights_0.y,
        weights_0.z,
        weights_0.w,
        weights_1.x,
        weights_1.y,
        weights_1.z,
        weights_1.w,
    );
    var attributes = geometry.attributes[vertex_index];
    for (var i = 0u; i < geometry.target_count; i++) {
        let weight = weights[i];
        let morph_attributes = geometry.attributes[(i + 1) * geometry.vertex_count + vertex_index];
        attributes.position += weight * morph_attributes.position;

        if contains(geometry_flags, geometry_normal_flag) {
            attributes.normal += weight * morph_attributes.normal;
            if contains(material_flags, material_normal_texture_flag) {
                attributes.tangent += vec4(weight * morph_attributes.tangent.xyz, 0.0);
            }
        }

        if contains(geometry_flags, geometry_tex_coord_0_flag) {
            attributes.tex_coord_0 += weight * morph_attributes.tex_coord_0;
        }
        if contains(geometry_flags, geometry_tex_coord_1_flag) {
            attributes.tex_coord_1 += weight * morph_attributes.tex_coord_1;
        }
        if contains(geometry_flags, geometry_color_0_flag) {
            attributes.color_0 += weight * morph_attributes.color_0;
        }
    }
    if contains(geometry_flags, geometry_color_0_flag) {
        attributes.color_0 = clamp(attributes.color_0, vec4(0.0), vec4(1.0));
    }

    var model_matrix: mat4x4<f32>;
    if joint_offset != 0 && contains(geometry_flags, geometry_weights_0_flag | geometry_joints_0_flag) {
        model_matrix = 
            attributes.weights_0.x * joint_matrices[joint_offset + attributes.joints_0.x] +
            attributes.weights_0.y * joint_matrices[joint_offset + attributes.joints_0.y] +
            attributes.weights_0.z * joint_matrices[joint_offset + attributes.joints_0.z] +
            attributes.weights_0.w * joint_matrices[joint_offset + attributes.joints_0.w];
    } else {
        model_matrix = mat4x4(
            model_col_0,
            model_col_1,
            model_col_2,
            model_col_3,
        );
    }

    let world_position = model_matrix * vec4(attributes.position, 1.0);

    var result: VertexOutput;
    result.position = camera.view_projection * world_position;
    result.world_position = world_position.xyz;

    if contains(geometry_flags, geometry_normal_flag) {
        let mat_x = model_matrix[0].xyz;
        let mat_y = model_matrix[1].xyz;
        let mat_z = model_matrix[2].xyz;
        let normal_matrix = mat3x3(
            cross(mat_y, mat_z),
            cross(mat_z, mat_x),
            cross(mat_x, mat_y),
        );

        result.world_normal = normal_matrix * determinant(model_matrix) * attributes.normal;
        if contains(material_flags, material_normal_texture_flag) {
            result.world_tangent = vec4(normal_matrix * attributes.tangent.xyz, attributes.tangent.w);
        }
    }
    
    if contains(geometry_flags, geometry_tex_coord_0_flag) {
        result.tex_coord_0 = attributes.tex_coord_0;
    }
    if contains(geometry_flags, geometry_tex_coord_1_flag) {
        result.tex_coord_1 = attributes.tex_coord_1;
    }
    if contains(geometry_flags, geometry_color_0_flag) {
        result.color_0 = attributes.color_0;
    }
    return result;
}

struct FragmentOutput {
    @location(0) opaque: vec4<f32>,
    @location(1) accumulation: vec4<f32>,
    @location(2) revealage: f32,
}

@fragment
fn fs_main(vertex: VertexOutput, @builtin(front_facing) front_facing: bool) -> FragmentOutput {
    var tex_coords = array(
        vertex.tex_coord_0,
        vertex.tex_coord_1,
    );

    var base_color = material_uniform.base_color_factor;
    if contains(geometry_flags, geometry_color_0_flag) {
        base_color *= vertex.color_0;
    }
    if contains(material_flags, material_base_color_texture_flag) {
        base_color *= textureSample(
            base_color_texture,
            base_color_sampler,
            tex_coords[material_uniform.base_color_tex_coord],
        );
    }

    var alpha: f32;
    if alpha_mode == 0 {
        // OPAQUE
        alpha = 1.0;
    } else if alpha_mode == 1 {
        // MASK
        if base_color.a >= material_uniform.alpha_cutoff {
            alpha = 1.0;
        } else {
            discard;
        }
    } else if alpha_mode == 2 {
        // BLEND
        alpha = base_color.a;
    }

    var emissive = material_uniform.emissive_factor;
    if contains(material_flags, material_emissive_texture_flag) {
        emissive *= textureSample(
            emissive_texture,
            emissive_sampler,
            tex_coords[material_uniform.emissive_tex_coord],
        ).rgb;
    }

    var color: vec4<f32>;
    if contains(geometry_flags, geometry_normal_flag) {
        let albedo = base_color.rgb;

        var metallic = material_uniform.metallic_factor;
        var roughness = material_uniform.roughness_factor;
        if contains(material_flags, material_metallic_roughness_texture_flag) {
            let metallic_roughness = textureSample(
                metallic_roughness_texture,
                metallic_roughness_sampler,
                tex_coords[material_uniform.metallic_roughness_tex_coord],
            );
            roughness *= metallic_roughness.g;
            metallic *= metallic_roughness.b;
        }

        var ambiance_occlusion = 1.0;
        if contains(material_flags, material_occlusion_texture_flag) {
            ambiance_occlusion = textureSample(
                occlusion_texture,
                occlusion_sampler,
                tex_coords[material_uniform.occlusion_tex_coord],
            ).r;
            ambiance_occlusion = 1.0 + material_uniform.occlusion_strength * (ambiance_occlusion - 1.0);
        }

        var normal = normalize(vertex.world_normal);
        if contains(material_flags, material_normal_texture_flag) {
            let tangent = normalize(vertex.world_tangent.xyz);
            let bitangent = cross(normal, tangent) * vertex.world_tangent.w;
            let tbn = mat3x3(tangent, bitangent, normal);
            normal = textureSample(
                normal_texture,
                normal_sampler,
                tex_coords[material_uniform.normal_tex_coord],
            ).rgb;
            normal = normal * 2.0 - 1.0;
            normal = normalize(tbn * normal);
        }
        if !front_facing {
            normal = -normal;
        }

        let view = normalize(camera.position - vertex.world_position);
        let reflected = reflect(-view, normal);

        var f0 = vec3(0.04);
        f0 = mix(f0, albedo, metallic);

        // reflectance equation
        var lo = vec3(0.0);
        for (var i = 0u; i < point_lights.count; i++) {
            // calculate per-light radiance
            let point_light = point_lights.data[i];
            let light   = normalize(point_light.position - vertex.world_position);
            let halfway = normalize(view + light);
            let distance    = length(point_light.position - vertex.world_position);
            let attenuation = 1.0 / (distance * distance);
            let radiance    = point_light.color * attenuation;

            // cook-torrance brdf
            let ndf = distributionGGX(normal, halfway, roughness);
            let g = geometrySmith(normal, view, light, roughness);
            let f = fresnelSchlick(max(dot(halfway, view), 0.0), f0);

            let ks = f;
            var kd = vec3(1.0) - ks;
            kd *= 1.0 - metallic;

            let numerator   = ndf * g * f;
            let denominator = 4.0 * max(dot(normal, view), 0.0) * max(dot(normal, light), 0.0) + 0.0001;
            let specular    = numerator / denominator;

            // add to outgoing radiance Lo
            let n_dot_l = max(dot(normal, light), 0.0);
            lo += (kd * albedo / pi + specular) * radiance * n_dot_l;
        }
    
        let f = fresnelSchlickRoughness(max(dot(normal, view), 0.0), f0, roughness);

        let ks = f;
        var kd = 1.0 - ks;
        kd *= 1.0 - metallic;

        let irradiance = textureSample(irradiance_map_texture, irradiance_map_sampler, normal).rgb;
        let diffuse = irradiance * albedo;

        let prefiltered_color = textureSampleLevel(
            prefilter_map_texture,
            prefilter_map_sampler,
            reflected,
            roughness * max_prefilter_map_mip,
        ).rgb;
        let env_brdf = textureSample(
            brdf_lut_texture,
            brdf_lut_sampler,
            vec2(max(dot(normal, view), 0.0), roughness),
        ).rg;
        let specular = prefiltered_color * (f * env_brdf.x + env_brdf.y);

        let ambient = (kd * diffuse + specular) * ambiance_occlusion;
        color = vec4(ambient + lo + emissive, alpha);
    } else {
        color = base_color + vec4(emissive, 0.0);
    }
    
    let brightness = dot(color.rgb, vec3(0.2126, 0.7152, 0.0722));

    var result: FragmentOutput;
    if alpha_mode == 0 || alpha_mode == 1 {
        result.opaque = color;
    } else if alpha_mode == 2 {
        let weight = clamp(pow(min(1.0, color.a * 10.0) + 0.01, 3.0) * 1e8 * 
                        pow(1.0 - vertex.position.z * 0.9, 3.0), 1e-2, 3e3);

        result.accumulation = vec4(color.rgb * alpha, alpha) * weight;
        result.revealage = alpha;
    }
    return result;
}

fn distributionGGX(normal: vec3<f32>, halfway: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let n_dot_h  = max(dot(normal, halfway), 0.0);
    let n_dot_h2 = n_dot_h * n_dot_h;
	
    let num   = a2;
    var denom = (n_dot_h2 * (a2 - 1.0) + 1.0);
    denom = pi * denom * denom;
	
    return num / denom;
}

fn geometrySchlickGGX(n_dot_v: f32, roughness: f32) -> f32 {
    let r = (roughness + 1.0);
    let k = (r*r) / 8.0;

    let num   = n_dot_v;
    let denom = n_dot_v * (1.0 - k) + k;
	
    return num / denom;
}

fn geometrySmith(normal: vec3<f32>, view: vec3<f32>, light: vec3<f32>, roughness: f32) -> f32 {
    let n_dot_v = max(dot(normal, view), 0.0);
    let n_dot_l = max(dot(normal, light), 0.0);
    let ggx2  = geometrySchlickGGX(n_dot_v, roughness);
    let ggx1  = geometrySchlickGGX(n_dot_l, roughness);
	
    return ggx1 * ggx2;
}

fn fresnelSchlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (1.0 - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

fn fresnelSchlickRoughness(cos_theta: f32, f0: vec3<f32>, roughness: f32) -> vec3<f32> {
    return f0 + (max(vec3(1.0 - roughness), f0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

fn contains(flags: u32, flag: u32) -> bool {
    return (flags & flag) == flag;
}