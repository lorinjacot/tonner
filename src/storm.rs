mod buffer;
mod camera;
mod material;
mod mesh;
mod mesh_old;
mod scene;
mod storage;
mod texture;
mod texture_old;

use std::path::Path;

use buffer::BufferManager;
use material::MaterialManager;
use mesh::MeshManager;
use scene::{Scene, SceneManager};
use storage::{Id, SparseSet};
use texture::TextureManager;

pub struct Storm {
    assets: SparseSet<Asset>,
    textures: TextureManager,
    materials: MaterialManager,
    buffers: BufferManager,
    meshes: MeshManager,
    scenes: SceneManager,
    active_scene: Option<Id<Scene>>,
}

impl Storm {
    pub fn new(render_format: wgpu::TextureFormat, device: &wgpu::Device) -> Self {
        let assets = SparseSet::new();
        let mut textures = TextureManager::new();
        let materials = MaterialManager::new(&mut textures, device);
        let mut buffers = BufferManager::new();
        let meshes = MeshManager::new(&materials, &mut buffers, render_format, device);
        let scenes = SceneManager::new();

        Self {
            assets,
            textures,
            materials,
            buffers,
            meshes,
            scenes,
            active_scene: None,
        }
    }

    pub fn load_asset(
        &mut self,
        path: impl AsRef<Path>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
    ) -> Result<Id<Asset>, gltf::Error> {
        let (document, buffers, images) = gltf::import(path)?;

        let id = self.assets.push(Asset { document });
        self.buffers.register_asset(id, buffers);
        self.textures.register_asset(id, images);

        let document = &self.assets[id].document;
        for scene in document.scenes() {
            self.scenes.load_scene(
                id,
                scene,
                &mut self.buffers,
                &mut self.textures,
                &mut self.materials,
                &mut self.meshes,
                device,
                queue,
            );
        }

        self.active_scene = document.default_scene().map(|scene| {
            self.scenes.load_scene(
                id,
                scene,
                &mut self.buffers,
                &mut self.textures,
                &mut self.materials,
                &mut self.meshes,
                device,
                queue,
            )
        });

        Ok(id)
    }

    pub fn render(&self, device: &wgpu::Device, render_pass: &mut wgpu::RenderPass) {
        let camera = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera uniform buffer"),
            size: 144,
            usage: wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Camera bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let camera = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera uniform bind group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera.as_entire_binding(),
            }],
        });
        if let Some(scene) = self.active_scene {
            self.scenes[scene].render(&camera, device, render_pass);
        }
    }
}

pub struct Asset {
    document: gltf::Document,
}
