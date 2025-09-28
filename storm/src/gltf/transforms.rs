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
