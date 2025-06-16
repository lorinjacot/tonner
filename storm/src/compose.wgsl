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

@group(0) @binding(0) var accumulation_texture: texture_2d<f32>;
@group(0) @binding(1) var revealage_texture: texture_2d<f32>;

@fragment
fn fs_main(
    @builtin(position) position: vec4<f32>
) -> @location(0) vec4<f32> {
    let coords = vec2<u32>(position.xy);

    let revealage = textureLoad(revealage_texture, coords, 0).r;

    // save the blending and color texture fetch cost if there is not a transparent fragment
    if isApproximatelyEqual(revealage, 1.0) {
        discard;
    }

    var accumulation = textureLoad(accumulation_texture, coords, 0);
    
    // suppress overflow
    if (isinf(max3(abs(accumulation.rgb)))) {
        accumulation = vec4(accumulation.a);
    }

    // prevent floating point precision bug
    let average_color = accumulation.rgb / max(accumulation.a, epsilon);

    return vec4(average_color, 1.0 - revealage);
}

// calculate floating point numbers equality accurately
fn isApproximatelyEqual(a: f32, b: f32) -> bool {
    return abs(a - b) <= max(abs(a), abs(b)) * epsilon;
}

fn max3(v: vec3<f32>) -> f32 {
    return max(max(v.x, v.y), v.z);
}

fn isinf(val: f32) -> bool {
    return abs(val) > 3.4028234e38;
}