use environment::{DrawEnvironment, Environment};
use light::{DrawLights, LightManager};
use material::MaterialManager;
use mesh::{DrawMeshes, MeshManager};
use node::NodeManager;

use crate::{
    camera::Camera,
    engine::DisplaySettings,
    texture::{
        Texture2d, Texture2dDescriptor, Texture2dSampler, Texture2dSource, TextureCreationError,
        TextureManager,
    },
};

mod environment;
mod light;
mod material;
mod mesh;
mod node;

pub use material::{MaterialDescriptor, MaterialId, NormalTextureDescriptor, TextureDescriptor};
pub use mesh::{
    MeshCreationError, MeshDescriptor, MeshId, PrimitiveAttributes, PrimitiveDescriptor,
    PrimitiveIndices,
};
pub use node::{NodeCreationError, NodeDescriptor, NodeId, Transform as NodeTransform};

pub struct Scene {
    nodes: NodeManager,
    meshes: MeshManager,
    materials: MaterialManager,
    lights: LightManager,
    textures: TextureManager,
    pub camera: Camera,
    environment: Environment,
}

impl Scene {
    pub fn new(camera: Camera, device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let mut textures = TextureManager::new(device.clone(), queue.clone());

        let nodes = NodeManager::new();

        let lights = LightManager::new(device, camera.bind_group_layout());

        let environment_image = image::ImageReader::open("assets/environments/Cannon_Exterior.hdr")
            .unwrap()
            .decode()
            .unwrap();
        let environment_texture = textures
            .create_from_image(
                Some("Environment texture"),
                wgpu::TextureUsages::TEXTURE_BINDING,
                &environment_image,
                false,
            )
            .unwrap();
        let environment_sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
        let equirectangular = Texture2dSampler {
            texture: environment_texture,
            sampler: environment_sampler,
        };

        let environment = Environment::from_equirectangular(
            &equirectangular,
            camera.bind_group_layout(),
            &mut textures,
            &device,
            &queue,
        );

        // let faces = [
        //     image::ImageReader::open("assets/environments/skybox/right.jpg")
        //         .unwrap()
        //         .decode()
        //         .unwrap(),
        //     image::ImageReader::open("assets/environments/skybox/left.jpg")
        //         .unwrap()
        //         .decode()
        //         .unwrap(),
        //     image::ImageReader::open("assets/environments/skybox/top.jpg")
        //         .unwrap()
        //         .decode()
        //         .unwrap(),
        //     image::ImageReader::open("assets/environments/skybox/bottom.jpg")
        //         .unwrap()
        //         .decode()
        //         .unwrap(),
        //     image::ImageReader::open("assets/environments/skybox/front.jpg")
        //         .unwrap()
        //         .decode()
        //         .unwrap(),
        //     image::ImageReader::open("assets/environments/skybox/back.jpg")
        //         .unwrap()
        //         .decode()
        //         .unwrap(),
        // ];

        // let environment = Environment::from_faces(
        //     &faces,
        //     camera.bind_group_layout(),
        //     &mut textures,
        //     &device,
        //     &queue,
        // )
        // .unwrap();

        let materials = MaterialManager::new(&mut textures, device.clone(), queue.clone());

        let meshes = MeshManager::new(
            device,
            camera.bind_group_layout(),
            lights.bind_group_layout(),
            materials.bind_group_layout(),
            environment.irradiance_map_bind_group_layout(),
        );

        Self {
            nodes,
            meshes,
            materials,
            lights,
            textures,
            camera,
            environment,
        }
    }

    pub fn create_node(
        &mut self,
        node: &NodeDescriptor,
        device: &wgpu::Device,
    ) -> Result<NodeId, NodeCreationError> {
        self.nodes.create(node, &mut self.meshes, device)
    }

    pub fn create_mesh(
        &mut self,
        mesh: MeshDescriptor,
        device: &wgpu::Device,
    ) -> Result<MeshId, MeshCreationError> {
        self.meshes.create(mesh, device)
    }

    pub fn create_material(&mut self, material: &MaterialDescriptor) -> MaterialId {
        self.materials.create(material)
    }

    pub fn create_texture2d(
        &mut self,
        texture: &Texture2dDescriptor,
    ) -> Result<Texture2d, TextureCreationError> {
        match texture.source {
            Texture2dSource::Pixel {
                width,
                height,
                pixels,
                format,
            } => Ok(self.textures.create_from_pixels(
                texture.label,
                texture.usage,
                width,
                height,
                pixels,
                format,
            )),
        }
    }

    pub fn update_buffers(&mut self, queue: &wgpu::Queue) {
        self.meshes.update_transforms(&self.nodes, queue);
    }
}

pub trait DrawScene {
    fn draw_scene(&mut self, scene: &Scene, display_setting: &DisplaySettings);
}

impl<'a> DrawScene for wgpu::RenderPass<'a> {
    fn draw_scene(&mut self, scene: &Scene, display_setting: &DisplaySettings) {
        self.draw_lights(&scene.lights, scene.camera.bind_group());

        self.draw_meshes(
            &scene.meshes,
            &scene.materials,
            scene.camera.bind_group(),
            scene.lights.bind_group(),
            scene.environment.irradiance_map_bind_group(),
        );

        self.draw_environment(
            &scene.environment,
            display_setting.background_blur,
            scene.camera.bind_group(),
        );
    }
}
