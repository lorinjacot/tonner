struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) tex_coord: vec3f,
}

@group(0) @binding(0) var<uniform> view_projection: mat4x4f;

@group(1) @binding(0) var equirectangular_texture: texture_2d<f32>;
@group(1) @binding(1) var equirectangular_sampler: sampler;

const inv_atan: vec2f = vec2f(0.1591, 0.3183);

@vertex
fn vs_cube(
    @location(0) position: vec3f
) -> VertexOutput {
    var result: VertexOutput;
    result.position = view_projection * vec4f(position, 1.0);
    result.tex_coord = vec3f(position.x, position.y, -position.z);
    return result;
}

@fragment
fn fs_equirectangular_to_cube(
    vertex: VertexOutput
) -> @location(0) vec4f {
    let normal = normalize(vertex.tex_coord);
    let tex_coord = 0.5 - vec2f(atan2(normal.z, normal.x), asin(normal.y)) * inv_atan;

    let color = textureSample(equirectangular_texture, equirectangular_sampler, tex_coord).rgb;
    return vec4f(color, 1.0);
}