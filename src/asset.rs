use std::path::Path;

use glam::{Mat4, Quat, Vec3};

use crate::{
    camera::Camera,
    scene::{NodeBuilder, NodeTransform, Scene},
};

pub struct Asset {
    pub document: gltf::Document,
    pub _buffers: Vec<gltf::buffer::Data>,
    _images: Vec<gltf::image::Data>,
}

impl Asset {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, gltf::Error> {
        let (document, _buffers, _images) = gltf::import(path)?;
        Ok(Self {
            document,
            _buffers,
            _images,
        })
    }

    pub fn create_scene(
        &self,
        gltf_scene: gltf::Scene,
        device: &wgpu::Device,
        camera: Camera,
    ) -> Result<Scene, ()> {
        let mut scene = Scene::new(device, camera);

        let nodes = gltf_scene.nodes().map(|node| self.create_node(&node));
        scene.create_node(nodes, device)?;

        Ok(scene)
    }

    fn create_node(&self, gltf_node: &gltf::Node) -> NodeBuilder {
        let transform = match gltf_node.transform() {
            gltf::scene::Transform::Decomposed {
                translation,
                rotation,
                scale,
            } => NodeTransform::TRS {
                translation: Vec3::from_array(translation),
                rotation: Quat::from_array(rotation),
                scale: Vec3::from_array(scale),
            },
            gltf::scene::Transform::Matrix { matrix } => {
                NodeTransform::Matrix(Mat4::from_cols_array_2d(&matrix))
            }
        };

        let children = gltf_node
            .children()
            .map(|child| self.create_node(&child))
            .collect();

        NodeBuilder::new()
            .set_transform(transform)
            .set_children(children)
    }
}
