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

struct Fragment {
    @builtin(position) position: vec4f,
    @location(0) world_position: vec3f,
    @location(1) world_normal: vec3f,
    @location(2) tex_coord_0: vec2f,
    @location(3) tex_coord_1: vec2f,
}

struct Light {
    color: vec3f,
    position: vec3f,
}

struct Material {
    base_color_factor: vec4f,
    base_color_tex_coord: u32,
}

@group(0) @binding(0) var<storage, read> transforms: array<Transform>;

@group(1) @binding(0) var<uniform> camera: Camera;

@group(2) @binding(0) var<uniform> light: Light;

@group(3) @binding(0) var base_color_texture: texture_2d<f32>;
@group(3) @binding(1) var base_color_sampler: sampler;
@group(3) @binding(2) var<uniform> material: Material;

@vertex
fn vs_main(
    @location(0) node_id: u32,
    attributes: Attributes
) -> Fragment {
    var fragment: Fragment;
    fragment.position = camera.view_projection * transforms[node_id].model * vec4f(attributes.position, 1.0);
    fragment.world_position = (transforms[node_id].model * vec4f(attributes.position, 1.0)).xyz;
    fragment.world_normal = transforms[node_id].normal * attributes.normal;
    fragment.tex_coord_0 = attributes.tex_coord_0;
    fragment.tex_coord_1 = attributes.tex_coord_1;
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

    let object_color = (material.base_color_factor * textureSample(
        base_color_texture, base_color_sampler, tex_coords[material.base_color_tex_coord]
    )).rgb;

    let ambientStrength = 0.1;
    let ambient = ambientStrength * light.color;

    let normal = normalize(fragment.world_normal);
    let light_dir = normalize(light.position - fragment.world_position);
    let diff = max(dot(normal, light_dir), 0.0);
    let diffuse = diff * light.color;

    let specular_strength = 0.5;
    let view_dir = normalize(camera.world_position - fragment.world_position);
    let halfwar_dir = normalize(light_dir + view_dir);
    let spec = pow(max(dot(normal, halfwar_dir), 0.0), 32.0);
    let specular = specular_strength * spec * light.color;

    let result = (ambient + diffuse + specular) * object_color;
    return vec4f(result, 1.0);
}