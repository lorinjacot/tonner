use std::ops::{Index, IndexMut};

use bytemuck::{Pod, Zeroable, bytes_of};
use storm::{
    DenseEntry, Id, Manager, ResourcesTrait,
    storage::{IntoIter, Iter, IterMut, SparseSet},
};
use wgpu::util::DeviceExt;

use crate::{
    MaterialBuilderTrait, MaterialManagerTrait, MaterialTrait, ResourcesRendererTrait,
    StormRendererTrait,
};

pub struct Material {
    id: Id<Self>,
    bind_group: wgpu::BindGroup,
    has_base_color_texture: bool,
    has_metallic_roughness_texture: bool,
}

impl DenseEntry for Material {
    type Key = Self;

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}

impl<Storm> MaterialTrait<Storm> for Material
where
    Storm: StormRendererTrait<Material = Self>,
{
    fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    fn has_base_color_texture(&self) -> bool {
        self.has_base_color_texture
    }

    fn has_metallic_roughness_texture(&self) -> bool {
        self.has_metallic_roughness_texture
    }
}

pub struct MaterialManager<Storm>
where
    Storm: StormRendererTrait<MaterialManager = Self>,
{
    materials: SparseSet<Storm::Material>,
    default_texture: wgpu::TextureView,
    default_sampler: wgpu::Sampler,
    material_bind_group_layout: wgpu::BindGroupLayout,
}

impl<Storm> Index<Id<Storm::Material>> for MaterialManager<Storm>
where
    Storm: StormRendererTrait<MaterialManager = Self>,
{
    type Output = Storm::Material;

    fn index(&self, index: Id<Storm::Material>) -> &Self::Output {
        &self.materials[index]
    }
}

impl<Storm> IndexMut<Id<Storm::Material>> for MaterialManager<Storm>
where
    Storm: StormRendererTrait<MaterialManager = Self>,
{
    fn index_mut(&mut self, index: Id<Storm::Material>) -> &mut Self::Output {
        &mut self.materials[index]
    }
}

impl<Storm> IntoIterator for MaterialManager<Storm>
where
    Storm: StormRendererTrait<MaterialManager = Self>,
{
    type Item = Storm::Material;
    type IntoIter = IntoIter<Storm::Material>;

    fn into_iter(self) -> Self::IntoIter {
        self.materials.into_iter()
    }
}

impl<Storm> Manager<Storm::Material> for MaterialManager<Storm>
where
    Storm: StormRendererTrait<MaterialManager = Self>,
{
    type Iter<'a> = Iter<'a, Storm::Material>;
    type IterMut<'a> = IterMut<'a, Storm::Material>;

    fn get(&self, id: Id<Storm::Material>) -> Option<&Storm::Material> {
        self.materials.get(id)
    }

    fn get_mut(&mut self, id: Id<Storm::Material>) -> Option<&mut Storm::Material> {
        self.materials.get_mut(id)
    }

    fn iter(&self) -> Self::Iter<'_> {
        self.materials.iter()
    }

    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.materials.iter_mut()
    }
}

impl<Storm> MaterialManagerTrait<Storm> for MaterialManager<Storm>
where
    Storm: StormRendererTrait<Material = Material, MaterialManager = Self>,
{
    fn new(device: &wgpu::Device) -> Self {
        let material_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Material bind group layout"),
                entries: &[
                    // Base color texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // metallic roughness texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // Material Uniform
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let default_texture = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Material default texture"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("Material default texture view"),
                ..Default::default()
            });

        let default_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Material default sampler"),
            ..Default::default()
        });

        Self {
            materials: SparseSet::new(),
            material_bind_group_layout,
            default_texture,
            default_sampler,
        }
    }

    fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.material_bind_group_layout
    }
}

#[must_use]
pub struct MaterialBuilder<'a, 'r, Storm>
where
    Storm: StormRendererTrait<MaterialBuilder<'a, 'r> = Self>,
{
    resources: &'r mut Storm::Resources,
    base_color_texture: Option<&'a wgpu::TextureView>,
    base_color_sampler: Option<&'a wgpu::Sampler>,
    metallic_roughness_texture: Option<&'a wgpu::TextureView>,
    metallic_roughness_sampler: Option<&'a wgpu::Sampler>,
    uniform: MaterialUniform,
}

impl<'a, 'r, Storm> MaterialBuilder<'a, 'r, Storm>
where
    Storm: StormRendererTrait<MaterialBuilder<'a, 'r> = Self>,
{
    pub fn base_color_factor(mut self, base_color_factor: [f32; 4]) -> Self {
        self.uniform.base_color_factor = base_color_factor;
        self
    }

    pub fn base_color_tex_coord(mut self, tex_coord: u32) -> Self {
        self.uniform.base_color_tex_coord = tex_coord;
        self
    }

    pub fn base_color_texture(mut self, texture: &'a wgpu::TextureView) -> Self {
        self.base_color_texture = Some(texture);
        self
    }

    pub fn base_color_sampler(mut self, sampler: &'a wgpu::Sampler) -> Self {
        self.base_color_sampler = Some(sampler);
        self
    }

    pub fn metallic_factor(mut self, metallic_factor: f32) -> Self {
        self.uniform.metallic_factor = metallic_factor;
        self
    }

    pub fn roughness_factor(mut self, roughness_factor: f32) -> Self {
        self.uniform.roughness_factor = roughness_factor;
        self
    }

    pub fn metallic_roughness_texture(mut self, texture: &'a wgpu::TextureView) -> Self {
        self.metallic_roughness_texture = Some(texture);
        self
    }

    pub fn metallic_roughness_sampler(mut self, sampler: &'a wgpu::Sampler) -> Self {
        self.metallic_roughness_sampler = Some(sampler);
        self
    }

    pub fn metallic_roughness_tex_coord(mut self, tex_coord: u32) -> Self {
        self.uniform.metallic_roughness_tex_coord = tex_coord;
        self
    }
}

impl<'a, 'r, Storm> MaterialBuilderTrait<'a, 'r, Storm> for MaterialBuilder<'a, 'r, Storm>
where
    Storm: StormRendererTrait<
            Material = Material,
            MaterialManager = MaterialManager<Storm>,
            MaterialBuilder<'a, 'r> = Self,
        >,
{
    fn new(resources: &'r mut <Storm>::Resources, _encoder: &'a mut wgpu::CommandEncoder) -> Self {
        let uniform = MaterialUniform {
            base_color_factor: [1.0; 4],
            base_color_tex_coord: 0,
            metallic_factor: 1.0,
            roughness_factor: 1.0,
            metallic_roughness_tex_coord: 0,
        };

        Self {
            resources,
            base_color_texture: None,
            base_color_sampler: None,
            metallic_roughness_texture: None,
            metallic_roughness_sampler: None,
            uniform,
        }
    }

    fn build(self) -> &'r <Storm as StormRendererTrait>::Material {
        let manager = self.resources.materials();
        let has_base_color_texture = self.base_color_texture.is_some();
        let has_metallic_roughness_texture = self.metallic_roughness_texture.is_some();
        let base_color_texture = self.base_color_texture.unwrap_or(&manager.default_texture);
        let base_color_sampler = self.base_color_sampler.unwrap_or(&manager.default_sampler);
        let metallic_roughness_texture = self
            .metallic_roughness_texture
            .unwrap_or(&manager.default_texture);
        let metallic_roughness_sampler = self
            .metallic_roughness_sampler
            .unwrap_or(&manager.default_sampler);

        let uniform_buffer =
            self.resources
                .device()
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Material uniform buffer"),
                    contents: bytes_of(&self.uniform),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
        let bind_group = self
            .resources
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Material bind gorup"),
                layout: &manager.material_bind_group_layout,
                entries: &[
                    // Base color texture
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(base_color_texture),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(base_color_sampler),
                    },
                    // metallic roughness texture
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(metallic_roughness_texture),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(metallic_roughness_sampler),
                    },
                    // Material uniform
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                ],
            });
        let id = manager.materials.next_id();
        self.resources.materials_mut().materials.insert(Material {
            id,
            bind_group,
            has_base_color_texture,
            has_metallic_roughness_texture,
        })
    }
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct MaterialUniform {
    base_color_factor: [f32; 4],
    base_color_tex_coord: u32,
    metallic_factor: f32,
    roughness_factor: f32,
    metallic_roughness_tex_coord: u32,
}
