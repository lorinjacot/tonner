struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec3<f32>,
}

@group(0) @binding(0) var<uniform> view_projection: mat4x4<f32>;

@vertex
fn vs_main(
    @location(0) position: vec3<f32>,
) -> VertexOutput {
    var result: VertexOutput;
    result.position = view_projection * vec4(position, 1.0);
    result.tex_coord = vec3(
        position.x,
        position.y,
        -position.z
    );
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
    let uv = sample_spherical_map(normalize(vertex.tex_coord));
    let color = textureSample(equirectangular_texture, equirectangular_sampler, uv).rgb;

    return vec4(color, 1.0);
}