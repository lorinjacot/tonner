const epsilon = 0.00001;

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32
) -> @builtin(position) vec4<f32> {
    let positions = array(
        vec4(-1.0,-1.0, 0.0, 1.0),
        vec4( 3.0,-1.0, 0.0, 1.0),
        vec4(-1.0, 3.0, 0.0, 1.0),
    );

    return positions[vertex_index];
}

@group(0) @binding(0) var opaque_texture: texture_2d<f32>;

@fragment
fn fs_main(
    @builtin(position) position: vec4<f32>
) -> @location(0) vec4<f32> {
    let coords = vec2<u32>(position.xy);

    let color = textureLoad(opaque_texture, coords, 0);
    let brightness = dot(color.rgb, vec3(0.2126, 0.7152, 0.0722));
    
    if brightness > 1.0 {
        return color;
    } else {
        return vec4(0.0, 0.0, 0.0, 1.0);
    }
}
