use crate::storage::{Id, Storage};

use super::texture::{Texture2d, TextureCube};

pub struct MaterialManager {
    materials: Storage<Material>,
    bind_group_layout: wgpu::BindGroupLayout,
    default_texture: Texture2d,
    default_normal_texture: TextureCube,
    default_sampler: wgpu::Sampler,
}

pub type MaterialId = Id<Material>;

pub struct Material {}
