struct Camera {
    view: mat4x4f,
    projection: mat4x4f,
}

struct Output {
    screen_position: vec2f,
    view_z: f32,
    visible: u32,
}

@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var depth_tex: texture_depth_2d;
@group(0) @binding(2) var depth_sampler: sampler_comparison;
@group(0) @binding(3) var<storage, read> world_positions: array<vec4f>;
@group(0) @binding(4) var<storage, read_write> outputs: array<Output>;

@compute
@workgroup_size(64)
fn main(
    @builtin(global_invocation_id) gid: vec3u,
) {
    let i = gid.x;
    if i >= arrayLength(&world_positions) {
        return;
    }

    // let world = vec4f(world_positions[i], 1.0);
    let world = world_positions[i];
    let view = camera.view * world;
    let clip = camera.projection * view;

    if clip.w <= 0.0 {
        outputs[i].visible = 0u;
        return;
    }

    let ndc = clip.xyz / clip.w;
    let uv = vec2f(
        (ndc.x + 1.0),
        (1.0 - ndc.y)
    ) * 0.5;

    let visible = textureSampleCompareLevel(depth_tex, depth_sampler, uv, ndc.z * 0.999);
    if visible > 0.5 {
        outputs[i].visible = 1u;

        let screen_dims = vec2f(textureDimensions(depth_tex));
        outputs[i].view_z = view.z;
        outputs[i].screen_position = uv * screen_dims;
    } else {
        outputs[i].visible = 0u;
    }
}