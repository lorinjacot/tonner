struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) tex_coord: vec2f,
}

@group(0) @binding(0) var hdr_texture: texture_2d<f32>;
@group(0) @binding(1) var hdr_sampler: sampler;
@group(0) @binding(2) var<uniform> exposure: f32;

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
) -> VertexOutput {
    let positions = array(
        vec4f(-1.0, 3.0, 0.0, 1.0),
        vec4f(-1.0,-1.0, 0.0, 1.0),
        vec4f( 3.0,-1.0, 0.0, 1.0),
    );
    let tex_coords = array(
        vec2f(0.0,-1.0),
        vec2f(0.0, 1.0),
        vec2f(2.0, 1.0),
    );
    
    var out: VertexOutput;
    out.position = positions[vertex_index];
    out.tex_coord = tex_coords[vertex_index];
    return out;
}

@fragment
fn fs_main(
    in: VertexOutput
) -> @location(0) vec4f {
    let hdr_color = textureSample(hdr_texture, hdr_sampler, in.tex_coord).rgb;

    let ldr_color = vec3(1.0) - exp(-hdr_color * exposure);
    // let ldr_color = hdr_color / (hdr_color + vec3f(1.0));

    return vec4f(ldr_color, 1.0);
}