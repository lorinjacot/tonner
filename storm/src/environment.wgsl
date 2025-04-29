const pi = 3.14159265359;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) local_position: vec3<f32>,
}

@group(0) @binding(0) var<uniform> view_projection: mat4x4<f32>;

@vertex
fn vs_main(
    @location(0) position: vec3<f32>,
) -> VertexOutput {
    var result: VertexOutput;
    result.position = view_projection * vec4(position, 1.0);
    result.local_position = position.xyz;
    return result;
}

@group(1) @binding(0) var equirectangular_texture: texture_2d<f32>;
@group(1) @binding(1) var equirectangular_sampler: sampler;

const inv_atan = vec2(0.1591, 0.3183);
fn sample_spherical_map(v: vec3<f32>) -> vec2<f32> {
    var uv = vec2(atan2(v.z, v.x), asin(v.y));
    uv *= inv_atan;
    uv = 0.5 - uv;
    return uv;
}

@fragment
fn fs_equirectangular_to_cubemap(
    vertex: VertexOutput,
) -> @location(0) vec4<f32> {
    let tex_coord = vec3(
        vertex.local_position.x,
        vertex.local_position.y,
        -vertex.local_position.z,
    );
    let uv = sample_spherical_map(normalize(tex_coord));
    let color = textureSample(equirectangular_texture, equirectangular_sampler, uv).rgb;

    return vec4(color, 1.0);
}

@group(1) @binding(0) var environment_cubemap_texture: texture_cube<f32>;
@group(1) @binding(1) var environment_cubemap_sampler: sampler;

@fragment
fn fs_irradiance(
    vertex: VertexOutput
) -> @location(0) vec4<f32> {
    // the sample direction equals the hemisphere's orientation
    let normal = normalize(vertex.local_position);

    var irradiance = vec3(0.0);
    var up = vec3(0.0, 1.0, 0.0);
    let right = normalize(cross(up, normal));
    up = normalize(cross(normal, right));

    let sample_delta = 0.025;
    var nr_samples = 0.0;
    for (var phi = 0.0; phi < 2.0 * pi; phi += sample_delta) {
        for (var theta = 0.0; theta < 0.5 * pi; theta += sample_delta) {
            // spherical to cartesian (in tangent space)
            let tangent_sample = vec3(sin(theta) * cos(phi),  sin(theta) * sin(phi), cos(theta));
            // tangent space to world
            var sample_vec = tangent_sample.x * right + tangent_sample.y * up + tangent_sample.z * normal;
            sample_vec.z = -sample_vec.z;

            irradiance += textureSample(environment_cubemap_texture, environment_cubemap_sampler, sample_vec).rgb
                            * cos(theta) * sin(theta);
            nr_samples = nr_samples + 1.0;
        }
    }
    irradiance = pi * irradiance * (1.0 / nr_samples);

    return vec4(irradiance, 1.0);
}