override has_base_color_texture: bool;
override has_metallic_roughness_texture: bool;
override has_normal_texture: bool;
override has_occlusion_texture: bool;
override has_emissive_texture: bool;

override max_prefilter_map_mip: f32;

const pi = 3.14159265359;
const max_weight_count = 8;

struct NodeUniform {
    matrix: mat4x4<f32>,
    weights: array<f32, max_weight_count>,
}

struct CameraUniform {
    view_projection: mat4x4<f32>,
    view: mat4x4<f32>,
    projection_inverse: mat4x4<f32>,
    position: vec3<f32>,
}

struct LightStorage {
    point_light_count: u32,
    point_lights: array<PointLight>,
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

@group(0) @binding(0) var<storage, read> nodes: array<NodeUniform>;
@group(0) @binding(1) var<uniform> camera: CameraUniform;
@group(0) @binding(2) var<storage, read> lights: LightStorage;
@group(0) @binding(3) var irradiance_map_texture: texture_cube<f32>;
@group(0) @binding(4) var irradiance_map_sampler: sampler;
@group(0) @binding(5) var prefilter_map_texture: texture_cube<f32>;
@group(0) @binding(6) var prefilter_map_sampler: sampler;
@group(0) @binding(7) var brdf_lut_texture: texture_2d<f32>;
@group(0) @binding(8) var brdf_lut_sampler: sampler;

struct Attribute {
    position: vec3<f32>,
    normal: vec3<f32>,
    tangent: vec4<f32>,
    tex_coord_0: vec2<f32>,
    tex_coord_1: vec2<f32>,
    color_0: vec4<f32>,
}

struct GeometryStorage {
    vertex_count: u32,
    target_count: u32,
    attributes: array<Attribute>,
}

@group(1) @binding(0) var<storage, read> geometry: GeometryStorage;

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) node_index: u32,
) -> VertexOutput {
    let node = nodes[node_index];

    var attributes = geometry.attributes[vertex_index];
    for (var i = 0u; i < geometry.target_count; i++) {
        let weight = node.weights[i];
        let morph_attributes = geometry.attributes[(i + 1) * geometry.vertex_count + vertex_index];
        attributes.position += weight * morph_attributes.position;
        attributes.normal += weight * morph_attributes.normal;
        if has_normal_texture {
            attributes.tangent += vec4(weight * morph_attributes.tangent.xyz, 0.0);
        }
        attributes.tex_coord_0 += weight * morph_attributes.tex_coord_0;
        attributes.tex_coord_1 += weight * morph_attributes.tex_coord_1;
        attributes.color_0 += weight * morph_attributes.color_0;
    }
    attributes.color_0 = clamp(attributes.color_0, vec4(0.0), vec4(1.0));

    let world_position = node.matrix * vec4(attributes.position, 1.0);

    let mat_x = node.matrix[0].xyz;
    let mat_y = node.matrix[1].xyz;
    let mat_z = node.matrix[2].xyz;
    let normal_matrix = mat3x3(
        cross(mat_y, mat_z),
        cross(mat_z, mat_x),
        cross(mat_x, mat_y),
    );

    var result: VertexOutput;
    result.position = camera.view_projection * world_position;
    result.world_position = world_position.xyz;
    result.world_normal = normal_matrix * attributes.normal;
    if has_normal_texture {
        result.world_tangent = vec4(normal_matrix * attributes.tangent.xyz, attributes.tangent.w);
    }
    result.tex_coord_0 = attributes.tex_coord_0;
    result.tex_coord_1 = attributes.tex_coord_1;
    result.color_0 = attributes.color_0;
    return result;
}

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @location(1) bright_color: vec4<f32>,
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
}

@group(2) @binding(0) var base_color_texture: texture_2d<f32>;
@group(2) @binding(1) var base_color_sampler: sampler;
@group(2) @binding(2) var metallic_roughness_texture: texture_2d<f32>;
@group(2) @binding(3) var metallic_roughness_sampler: sampler;
@group(2) @binding(4) var normal_texture: texture_2d<f32>;
@group(2) @binding(5) var normal_sampler: sampler;
@group(2) @binding(6) var occlusion_texture: texture_2d<f32>;
@group(2) @binding(7) var occlusion_sampler: sampler;
@group(2) @binding(8) var emissive_texture: texture_2d<f32>;
@group(2) @binding(9) var emissive_sampler: sampler;
@group(2) @binding(10) var<uniform> material: MaterialUniform;

@fragment
fn fs_main(vertex: VertexOutput) -> FragmentOutput {
    var tex_coords = array(
        vertex.tex_coord_0,
        vertex.tex_coord_1,
    );

    var base_color = material.base_color_factor * vertex.color_0;
    if has_base_color_texture {
        base_color *= textureSample(
            base_color_texture,
            base_color_sampler,
            tex_coords[material.base_color_tex_coord],
        );
    }

    let albedo = base_color.rgb;

    var metallic = material.metallic_factor;
    var roughness = material.roughness_factor;
    if has_metallic_roughness_texture {
        let metallic_roughness = textureSample(
            metallic_roughness_texture,
            metallic_roughness_sampler,
            tex_coords[material.metallic_roughness_tex_coord],
        );
        roughness *= metallic_roughness.g;
        metallic *= metallic_roughness.b;
    }

    var ambiance_occlusion = 1.0;
    if has_occlusion_texture {
        ambiance_occlusion = textureSample(
            occlusion_texture,
            occlusion_sampler,
            tex_coords[material.occlusion_tex_coord],
        ).r;
        ambiance_occlusion = 1.0 + material.occlusion_strength * (ambiance_occlusion - 1.0);
    }

    var emissive = material.emissive_factor;
    if has_emissive_texture {
        emissive *= textureSample(
            emissive_texture,
            emissive_sampler,
            tex_coords[material.emissive_tex_coord],
        ).rgb;
    }

    var normal = normalize(vertex.world_normal);
    if has_normal_texture {
        let tangent = normalize(vertex.world_tangent.xyz);
        let bitangent = cross(normal, tangent) * vertex.world_tangent.w;
        let tbn = mat3x3(tangent, bitangent, normal);
        normal = textureSample(
            normal_texture,
            normal_sampler,
            tex_coords[material.normal_tex_coord],
        ).rgb;
        normal = normal * 2.0 - 1.0;
        normal = normalize(tbn * normal);
    }
    let view = normalize(camera.position - vertex.world_position);
    let reflected = reflect(-view, normal);

    var f0 = vec3(0.04);
    f0 = mix(f0, albedo, metallic);

    // reflectance equation
    var lo = vec3(0.0);
    for (var i = 0u; i < lights.point_light_count; i++) {
        // calculate per-light radiance
        let point_light = lights.point_lights[i];
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
    let color = vec4(ambient + lo + emissive, base_color.a);
    let brightness = dot(color.rgb, vec3(0.2126, 0.7152, 0.0722));

    var result: FragmentOutput;
    result.color = color;
    if brightness > 1.0 {
        result.bright_color = color;
    } else {
        result.bright_color = vec4(0.0, 0.0, 0.0, color.a);
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
