use glam::{UVec4, Vec2, Vec3, Vec4, uvec4, vec2, vec3, vec4};

pub fn i8x1_to_f32(a: &[i8; 1]) -> f32 {
    (a[0] as f32 / 127.0).max(-1.0)
}

pub fn i8x2_to_vec2(a: &[i8; 2]) -> Vec2 {
    (vec2(a[0] as f32, a[1] as f32) / 127.0).max(vec2(-1.0, -1.0))
}

pub fn i8x3_to_vec3(a: &[i8; 3]) -> Vec3 {
    (vec3(a[0] as f32, a[1] as f32, a[2] as f32) / 127.0).max(vec3(-1.0, -1.0, -1.0))
}

pub fn i8x4_to_vec4(a: &[i8; 4]) -> Vec4 {
    (vec4(a[0] as f32, a[1] as f32, a[2] as f32, a[3] as f32) / 127.0)
        .max(vec4(-1.0, -1.0, -1.0, -1.0))
}

pub fn i8x4_to_f32x4(a: &[i8; 4]) -> [f32; 4] {
    i8x4_to_vec4(a).to_array()
}

pub fn u8x1_to_f32(a: &[u8; 1]) -> f32 {
    a[0] as f32 / 255.0
}

pub fn u8x2_to_vec2(a: &[u8; 2]) -> Vec2 {
    vec2(a[0] as f32, a[1] as f32) / 255.0
}

pub fn u8x3_to_vec3(a: &[u8; 3]) -> Vec3 {
    vec3(a[0] as f32, a[1] as f32, a[2] as f32) / 255.0
}

pub fn u8x4_to_vec4(a: &[u8; 4]) -> Vec4 {
    vec4(a[0] as f32, a[1] as f32, a[2] as f32, a[3] as f32) / 255.0
}

pub fn u8x4_to_f32x4(a: &[u8; 4]) -> [f32; 4] {
    u8x4_to_vec4(a).to_array()
}

pub fn i16x1_to_f32(a: &[i16; 1]) -> f32 {
    (a[0] as f32 / 32767.0).max(-1.0)
}

pub fn i16x2_to_vec2(a: &[i16; 2]) -> Vec2 {
    (vec2(a[0] as f32, a[1] as f32) / 32767.0).max(vec2(-1.0, -1.0))
}

pub fn i16x3_to_vec3(a: &[i16; 3]) -> Vec3 {
    (vec3(a[0] as f32, a[1] as f32, a[2] as f32) / 32767.0).max(vec3(-1.0, -1.0, -1.0))
}

pub fn i16x4_to_vec4(a: &[i16; 4]) -> Vec4 {
    (vec4(a[0] as f32, a[1] as f32, a[2] as f32, a[3] as f32) / 32767.0)
        .max(vec4(-1.0, -1.0, -1.0, -1.0))
}

pub fn i16x4_to_f32x4(a: &[i16; 4]) -> [f32; 4] {
    i16x4_to_vec4(a).to_array()
}

pub fn u16x1_to_f32(a: &[u16; 1]) -> f32 {
    a[0] as f32 / 65535.0
}

pub fn u16x2_to_vec2(a: &[u16; 2]) -> Vec2 {
    vec2(a[0] as f32, a[1] as f32) / 65535.0
}

pub fn u16x3_to_vec3(a: &[u16; 3]) -> Vec3 {
    vec3(a[0] as f32, a[1] as f32, a[2] as f32) / 65535.0
}

pub fn u16x4_to_vec4(a: &[u16; 4]) -> Vec4 {
    vec4(a[0] as f32, a[1] as f32, a[2] as f32, a[3] as f32) / 65535.0
}

pub fn u16x4_to_f32x4(a: &[u16; 4]) -> [f32; 4] {
    u16x4_to_vec4(a).to_array()
}

pub fn u8x4_to_uvec4(a: &[u8; 4]) -> UVec4 {
    uvec4(a[0] as u32, a[1] as u32, a[2] as u32, a[3] as u32)
}

pub fn u16x4_to_uvec4(a: &[u16; 4]) -> UVec4 {
    uvec4(a[0] as u32, a[1] as u32, a[2] as u32, a[3] as u32)
}

pub fn is_0(value: &usize) -> bool {
    *value == 0
}

pub fn is_false(value: &bool) -> bool {
    *value == false
}

pub fn is_3x00(value: &[f32; 3]) -> bool {
    *value == [0.0; 3]
}

pub fn default_05() -> f32 {
    0.5
}

pub fn is_05(value: &f32) -> bool {
    *value == 0.5
}

pub fn default_10() -> f32 {
    1.0
}

pub fn is_10(value: &f32) -> bool {
    *value == 1.0
}

pub fn default_4x10() -> [f32; 4] {
    [1.0; 4]
}

pub fn is_4x10(value: &[f32; 4]) -> bool {
    *value == [1.0; 4]
}
