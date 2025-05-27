use storm::{DenseEntry, Manager, ResourcesTrait, StormTrait};

pub mod material;
pub mod mesh;

pub trait StormRendererTrait: StormTrait<Resources: ResourcesRendererTrait<Self>> {
    type Material: MaterialTrait<Self>;
    type MaterialManager: MaterialManagerTrait<Self>;
    type MaterialBuilder<'a, 'r>: MaterialBuilderTrait<'a, 'r, Self>;

    type Mesh: MeshTrait<Self>;
    type MeshManager: MeshManagerTrait<Self>;
    type MeshBuilder<'a, 'r>: MeshBuilderTrait<'a, 'r, Self>;
}

pub trait ResourcesRendererTrait<Storm>: ResourcesTrait<Storm>
where
    Storm: StormRendererTrait<Resources = Self>,
{
    fn render_texture_format(&self) -> wgpu::TextureFormat;

    fn materials(&self) -> &Storm::MaterialManager;
    fn materials_mut(&mut self) -> &mut Storm::MaterialManager;
    fn material_builder<'a, 'r>(
        &'r mut self,
        encoder: &'a mut wgpu::CommandEncoder,
    ) -> Storm::MaterialBuilder<'a, 'r> {
        Storm::MaterialBuilder::new(self, encoder)
    }

    fn meshes(&self) -> &Storm::MeshManager;
    fn meshes_mut(&mut self) -> &mut Storm::MeshManager;
    fn mesh_builder<'a, 'r>(
        &'r mut self,
        encoder: &'a mut wgpu::CommandEncoder,
    ) -> Storm::MeshBuilder<'a, 'r> {
        Storm::MeshBuilder::new(self, encoder)
    }
}

pub trait MaterialTrait<Storm>: DenseEntry<Key = Self> + 'static
where
    Storm: StormRendererTrait<Material = Self>,
{
    fn has_base_color_texture(&self) -> bool;
    fn has_metallic_roughness_texture(&self) -> bool;

    fn bind_group(&self) -> &wgpu::BindGroup;
}

pub trait MaterialManagerTrait<Storm>: Manager<Storm::Material>
where
    Storm: StormRendererTrait<MaterialManager = Self>,
{
    fn new(device: &wgpu::Device) -> Self;

    fn bind_group_layout(&self) -> &wgpu::BindGroupLayout;
}

pub trait MaterialBuilderTrait<'a, 'r, Storm>
where
    Storm: StormRendererTrait<MaterialBuilder<'a, 'r> = Self>,
{
    fn new(resources: &'r mut Storm::Resources, encoder: &'a mut wgpu::CommandEncoder) -> Self;

    fn build(self) -> &'r Storm::Material;
}

pub trait MeshTrait<Storm>: DenseEntry<Key = Self> + 'static
where
    Storm: StormRendererTrait<Mesh = Self>,
{
}

pub trait MeshManagerTrait<Storm>: Manager<Storm::Mesh>
where
    Storm: StormRendererTrait<MeshManager = Self>,
{
    fn new(
        device: &wgpu::Device,
        scene_bind_group_layout: &wgpu::BindGroupLayout,
        material_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self;
}

pub trait MeshBuilderTrait<'a, 'r, Storm>
where
    Storm: StormRendererTrait<MeshBuilder<'a, 'r> = Self>,
{
    fn new(resources: &'r mut Storm::Resources, encoder: &'a mut wgpu::CommandEncoder) -> Self;

    fn build(self) -> &'r mut Storm::Mesh;
}
