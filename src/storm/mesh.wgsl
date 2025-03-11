override has_normal: bool;
override has_tangent: bool;
override has_tex_coord_0: bool;
override has_tex_coord_1: bool;
override has_color_0: bool;

struct Attributes {
    @location(0) position: vec3f,
    @location(1) normal: vec3f,
    @location(2) tangent: vec4f,
    @location(3) tex_coord_0: vec2f,
    @location(4) tex_coord_1: vec2f,
    @location(5) color_0: vec4f,
}

const position_offset = 0 * 4;
const normal_offset = 3 * 4;
const tangent_offset = 6 * 4;
const tex_coord_0_offset = 10 * 4;
const tex_coord_1_offset = 12 * 4;
const color_0_offset = 14 * 4;

@group(0) @binding(0) var<storage, read_write> vertex_buffer: array<f32>;

@vertex
fn vs_attributes(
    @builtin(vertex_index): vertex_index: u32,
    attributes: Attributes,
) -> @builtin(position) vec4f {
    let vertex_offset = 18 * 4 * vertex_index;

    set(vertex_offset + position_offset, attributes.position);

    if has_normal {
        set(vertex_offset + normal_offset, attributes.normal);
    }

    if has_tangent {
        set(vertex_offset + tangent_offset, attributes.tangent);
    }

    if has_tex_coord_0 {
        set(vertex_offset + tex_coord_0_offset, attributes.tex_coord_0);
    }
    if has_tex_coord_1 {
        set(vertex_offset + tex_coord_1_offset, attributes.tex_coord_1);
    }

    if has_color_0 {
        set(vertex_offset + color_0_offset, attributes.color_0);
    }
}

fn set(index: u32, value: vec2f) {
    vertex_buffer[index] = value.x;
    vertex_buffer[index + 1] = value.y;
}

fn set(index: u32, value: vec3f) {
    vertex_buffer[index] = value.x;
    vertex_buffer[index + 1] = value.y;
    vertex_buffer[index + 2] = value.z;
}

fn set(index: u32, value: vec4f) {
    vertex_buffer[index] = value.x;
    vertex_buffer[index + 1] = value.y;
    vertex_buffer[index + 2] = value.z;
    vertex_buffer[index + 3] = value.w;
}