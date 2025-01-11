struct Attributes {
    @location(1) position: vec3f,
    @location(2) normal: vec3f,
}

struct Transform {
    model: mat4x4f,
    normal: mat3x3f,
}

struct Camera {
    view_projection: mat4x4f,
    world_position: vec3f,
}

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) world_position: vec3f,
    @location(1) world_normal: vec3f,
}

struct Light {
    color: vec3f,
    position: vec3f,
}

@group(0) @binding(0) var<storage, read> transforms: array<Transform>;

@group(1) @binding(0) var<uniform> camera: Camera;

@group(2) @binding(0) var<uniform> light: Light;

@vertex
fn vs_main(
    @location(0) node_id: u32,
    attributes: Attributes
) -> VertexOutput {
    var out: VertexOutput;
    out.position = camera.view_projection * transforms[node_id].model * vec4f(attributes.position, 1.0);
    out.world_position = (transforms[node_id].model * vec4f(attributes.position, 1.0)).xyz;
    out.world_normal = transforms[node_id].normal * attributes.normal;
    return out;
}

@fragment
fn fs_main(
    in: VertexOutput
) -> @location(0) vec4f {
    let object_color = vec3f(1.0, 0.0, 0.0);

    let ambientStrength = 0.1;
    let ambient = ambientStrength * light.color;

    let norm = normalize(in.world_normal);
    let light_dir = normalize(light.position - in.world_position);
    let diff = max(dot(norm, light_dir), 0.0);
    let diffuse = diff * light.color;

    let specular_strength = 0.5;
    let view_dir = normalize(camera.world_position - in.world_position);
    let reflect_dir = reflect(-light_dir, norm);
    let spec = pow(max(dot(view_dir, reflect_dir), 0.0), 32.0);
    let specular = specular_strength * spec * light.color;

    let result = (ambient + diffuse + specular) * object_color;
    return vec4f(result, 1.0);
}