struct Fragment {
    @builtin(position) position: vec4f,
    @location(0) normal: vec3f,
}

const inv_atan: vec2f = vec2f(0.1591, 0.3183);

@group(0) @binding(0) var<uniform> view_projection: mat4x4f;

@group(1) @binding(0) var equirectangular_texture: texture_2d<f32>;
@group(1) @binding(1) var equirectangular_sampler: sampler;

@vertex
fn vs_main(
    @location(0) position: vec3f,
) -> Fragment {
    var fragment: Fragment;
    fragment.position = view_projection * vec4f(position, 1.0);
    fragment.normal = position;
    return fragment;
}

@fragment
fn fs_main(
    fragment: Fragment
) -> @location(0) vec4f {
    let normal = normalize(fragment.normal);

    let tex_coord = vec2f(atan2(normal.z, normal.x), asin(normal.y)) * inv_atan + 0.5;

    let color = textureSample(equirectangular_texture, equirectangular_sampler, tex_coord).rgb;

    return vec4f(color, 1.0);
}
