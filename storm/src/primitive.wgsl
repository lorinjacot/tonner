const pi = 3.14159265359;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(1) world_position: vec3<f32>,
    @location(2) world_normal: vec3<f32>,
}

struct Attributes {
    @location(1) position: vec3<f32>,
    @location(2) normal: vec3<f32>,
}

struct NodeUniform {
    model: mat4x4<f32>,
    model_normal: mat3x3<f32>,
}

struct CameraUniform {
    view_projection: mat4x4<f32>,
    view_projection_inv: mat4x4<f32>,
    position: vec3<f32>,
}

@group(0) @binding(0) var<storage, read> nodes: array<NodeUniform>;
@group(0) @binding(1) var<uniform> camera: CameraUniform;
@group(0) @binding(2) var irradiance_map_texture: texture_cube<f32>;
@group(0) @binding(3) var irradiance_map_sampler: sampler;

struct MaterialUniform {
    base_color_factor: vec4<f32>,
    metallic_factor: f32,
    roughness_factor: f32,
}

@group(1) @binding(0) var<uniform> material: MaterialUniform;

@vertex
fn vs_main(
    @location(0) index: u32,
    attributes: Attributes,
) -> VertexOutput {
    let node = nodes[index];

    var result: VertexOutput;
    result.position = camera.view_projection * node.model * vec4(attributes.position, 1.0);
    result.world_normal = node.model_normal * attributes.normal;
    return result;
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(vertex.world_normal);
    let view = normalize(camera.position - vertex.world_position);

    let albedo = material.base_color_factor.rgb;
    let metallic = material.metallic_factor;
    let roughness = material.roughness_factor;
    let ambiance_occlusion = 1.0;

    var f0 = vec3(0.04);
    f0 = mix(f0, albedo, metallic);
	           
    // reflectance equation
    var lo = vec3(0.0);
    // todo: iterate over lights
  
    let ks = fresnelSchlickRoughness(max(dot(normal, view), 0.0), f0, roughness);
    let kd = 1.0 - ks;
    let irradiance = textureSample(irradiance_map_texture, irradiance_map_sampler, normal).rgb;
    let diffuse = irradiance * albedo;
    let ambient = (kd * diffuse) * ambiance_occlusion;
    var color = ambient + lo;

    color = color / (color + vec3(1.0));

    return vec4(color, 1.0);
}

fn distributionGGX(normal: vec3<f32>, halfway: vec3<f32>, roughness: f32) -> f32
{
    let a      = roughness*roughness;
    let a2     = a*a;
    let n_dot_h  = max(dot(normal, halfway), 0.0);
    let n_dot_h2 = n_dot_h*n_dot_h;
	
    let num   = a2;
    var denom = (n_dot_h2 * (a2 - 1.0) + 1.0);
    denom = pi * denom * denom;
	
    return num / denom;
}

fn geometrySchlickGGX(n_dot_v: f32, roughness: f32) -> f32
{
    let r = (roughness + 1.0);
    let k = (r*r) / 8.0;

    let num   = n_dot_v;
    let denom = n_dot_v * (1.0 - k) + k;
	
    return num / denom;
}
fn geometrySmith(normal: vec3<f32>, view: vec3<f32>, light: vec3<f32>, roughness: f32) -> f32
{
    let n_dot_v = max(dot(normal, view), 0.0);
    let n_dot_l = max(dot(normal, light), 0.0);
    let ggx2  = geometrySchlickGGX(n_dot_v, roughness);
    let ggx1  = geometrySchlickGGX(n_dot_l, roughness);
	
    return ggx1 * ggx2;
}

fn fresnelSchlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32>
{
    return f0 + (1.0 - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

fn fresnelSchlickRoughness(cos_theta: f32, f0: vec3<f32>, roughness: f32) -> vec3<f32>
{
    return f0 + (max(vec3(1.0 - roughness), f0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}   