const position_offset = 0u;
const normal_offset = 12u;
const tangent_offset = 24u;
const tex_coord_0_offset = 40u;
const tex_coord_1_offset = 48u;
const color_0_offset = 56u;
const attribute_stride = 72u;

const position_index = 0u;
const normal_index = 1u;
const tangent_index = 2u;
const tex_coord_0_index = 3u;
const tex_coord_1_index = 4u;
const color_0_index = 5u;

const i8_component_type = 5120u;
const u8_component_type = 5121u;
const i16_component_type = 5122u;
const u16_component_type = 5123u;
const u32_component_type = 5125u;
const f32_component_type = 5126u;

struct Accessor {
    offset: u32,
    component_type: u32,
    component_number: u32,
    stride: u32,
}

@group(0) @binding(0) var<storage, read> positions: array<f32>;
@group(0) @binding(1) var<storage, read> normals: array<f32>;
@group(0) @binding(2) var<storage, read> tangents: array<f32>;
@group(0) @binding(3) var<storage, read> tex_coords_0: array<u32>;

@group(1) @binding(0) var<storage, read> tex_coords_1: array<u32>;
@group(1) @binding(1) var<storage, read> colors_0: array<u32>;
@group(1) @binding(2) var<storage, read_write> attributes: array<u32>;
@group(1) @binding(3) var<uniform> accessors: array<Accessor, 6>;

@compute
@workgroup_size(64)
fn writeAttributes(
    @builtin(global_invocation_id) global_invocation_id: vec3<u32>,
    @builtin(num_workgroups) num_workgroups: vec3<u32>,
) {
    let index = global_invocation_id.x + (global_invocation_id.y * 64u * num_workgroups.x) + (global_invocation_id.z * 64u * num_workgroups.x * 64u * num_workgroups.y);
    if index >= arrayLength(&attributes) {
        return;
    }
    
    // POSITION: vec3<f32>
    {
        let start_att = attribute_stride * index + position_offset;
        let accessor = accessors[position_index];
        let start_buff = (index * accessor.stride + accessor.offset) / 4u;
        for (var i = 0u; i < 3u; i++) {
            writeAttribute_f32(start_att + i, positions[start_buff + i]);
        }
    }
    
    // NORMAL: vec3<f32>
    {
        let start_att = attribute_stride * index + normal_offset;
        let accessor = accessors[normal_index];
        let start_buff = (index * accessor.stride + accessor.offset) / 4u;
        for (var i = 0u; i < 3u; i++) {
            writeAttribute_f32(start_att + i, normals[start_buff + i]);
        }
    }
    
    // TANGENT: vec4<f32>
    {
        let start_att = attribute_stride * index + tangent_offset;
        let accessor = accessors[tangent_index];
        let start_buff = (index * accessor.stride + accessor.offset) / 4u;
        for (var i = 0u; i < 4u; i++) {
            writeAttribute_f32(start_att + i, tangents[start_buff + i]);
        }
    }
    
    // TEXCOORD_0: vec2<f32>, vec2<u8> or vec2<u16>
    {
        let start_att = attribute_stride * index + tex_coord_0_offset;
        let accessor = accessors[tex_coord_0_index];
        let start_buff = (index * accessor.stride + accessor.offset) / 4u;
        if accessor.component_type == u32_component_type {
            for (var i = 0u; i < 2u; i++) {
                writeAttribute_u32(start_att + i, tex_coords_0[start_buff + i]);
            }
        } else if accessor.component_type == u8_component_type {
            let value = unpack4x8unorm(tex_coords_0[start_buff]);
            writeAttribute_vec2f(start_att, value.xy);
        } else {
            let value = unpack2x16unorm(tex_coords_0[start_buff]);
            writeAttribute_vec2f(start_att, value.xy);
        }
    }
    
    // TEXCOORD_1: vec2<f32>, vec2<u8> or vec2<u16>
    {
        let start_att = attribute_stride * index + tex_coord_1_offset;
        let accessor = accessors[tex_coord_1_index];
        let start_buff = (index * accessor.stride + accessor.offset) / 4u;
        if accessor.component_type == u32_component_type {
            for (var i = 0u; i < 2u; i++) {
                writeAttribute_u32(start_att + i, tex_coords_1[start_buff + i]);
            }
        } else if accessor.component_type == u8_component_type {
            let value = unpack4x8unorm(tex_coords_1[start_buff]);
            writeAttribute_vec2f(start_att, value.xy);
        } else {
            let value = unpack2x16unorm(tex_coords_1[start_buff]);
            writeAttribute_vec2f(start_att, value.xy);
        }
    }
    
    // COLOR_0: vec3<f32>, vec4<f32>, vec3<u8>, vec4<u8>, vec3<u16>, vec4<u16>
    {
        let start_att = attribute_stride * index + color_0_offset;
        let accessor = accessors[color_0_index];
        let start_buff = (index * accessor.stride + accessor.offset) / 4u;
        if accessor.component_type == u32_component_type {
            for (var i = 0u; i < 2u; i++) {
                writeAttribute_u32(start_att + i, colors_0[start_buff + i]);
            }
        } else if accessor.component_type == u8_component_type {
            let value = unpack4x8unorm(colors_0[start_buff]);
            writeAttribute_vec4f(start_att, value);
        } else {
            for (var i = 0u; i < 2u; i++) {
                let value = unpack2x16unorm(colors_0[start_buff + i]);
                writeAttribute_vec2f(start_att + 2u * i, value.xy);
            }
        }
        if accessor.component_number == 3u {
            writeAttribute_f32(start_att + 3u, 1.0);
        }
    }
}

fn writeAttribute_u32(index: u32, value: u32) {
    attributes[index] = value;
}

fn writeAttribute_f32(index: u32, value: f32) {
    attributes[index] = bitcast<u32>(value);
}

fn writeAttribute_vec2f(index: u32, value: vec2<f32>) {
    let bytes = bitcast<vec2<u32>>(value);
    attributes[index] = bytes.x;
    attributes[index + 1u] = bytes.y;
}

fn writeAttribute_vec4f(index: u32, value: vec4<f32>) {
    let bytes = bitcast<vec4<u32>>(value);
    attributes[index] = bytes.x;
    attributes[index + 1u] = bytes.y;
    attributes[index + 2u] = bytes.z;
    attributes[index + 3u] = bytes.w;
}