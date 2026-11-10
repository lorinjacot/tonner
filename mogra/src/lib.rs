use crate::{
    camera::{Camera, CameraManager},
    mesh::{MeshInstance, MeshManager},
    primitive::PrimitiveRenderer,
    scene_graph::{SceneGraph, SiblingNodeIter},
};
use glam::{Quat, Vec3};
pub use node::NodeBuilder;
pub use scene_graph::{
    AllChildrenNodeIter, DirectChildrenNodeIter, Node, NodeId, NodeIter, ParentNodeIter,
    RootNodeIter,
};

pub mod camera;
pub mod geometry;
pub mod mesh;
mod node;
mod primitive;
mod scene_graph;

mod post_processing {
    pub use bloom::*;
    pub use tone_mapping::*;

    mod bloom;
    mod tone_mapping;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    device: wgpu::Device,
    queue: wgpu::Queue,
    bloom: post_processing::BloomContext,
    tone_mapping: post_processing::ToneMappingContext,
}

impl Context {
    /// Create the context using the provided `device` and `queue`. All GPU resourcces and
    /// GPU operations will be created on them.
    pub fn from_device(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let bloom = post_processing::BloomContext::new(&device);
        let tone_mapping = post_processing::ToneMappingContext::new(&device);
        Context {
            device,
            queue,
            bloom,
            tone_mapping,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Scene {
    ctx: Context,
    scene_graph: SceneGraph,
    meshes: MeshManager,
    cameras: CameraManager,
}

impl Scene {
    pub fn new(ctx: Context) -> Self {
        let scene_graph = SceneGraph::new();
        let meshes = MeshManager::new(&ctx);
        let cameras = CameraManager::new();
        Scene {
            ctx,
            scene_graph,
            meshes,
            cameras,
        }
    }

    pub fn context(&self) -> &Context {
        &self.ctx
    }

    /// Renders the scene to the given target from the `camera` point of view using the provided renderer.
    ///
    /// # Panics
    /// This function will panic if:
    /// - the renderer and the target have different dimensions;
    /// - the renderer and the target have different texture format;
    /// - the scene and the render are using different [Context]s.
    pub fn render(&self, camera: NodeId, target: &wgpu::TextureView, renderer: &mut Renderer) {
        renderer.render(self, camera, target)
    }

    pub fn contains(&self, node: NodeId) -> bool {
        self.scene_graph.contains(node)
    }

    pub fn node(&self, node: NodeId) -> Option<&Node> {
        self.scene_graph.get(node)
    }

    pub fn nodes<'a>(&'a self) -> NodeIter<'a> {
        self.scene_graph.iter()
    }

    pub fn root_nodes<'a>(&'a self) -> RootNodeIter<'a> {
        self.scene_graph.roots()
    }

    pub fn parent_nodes<'a>(&'a self, node: NodeId) -> ParentNodeIter<'a> {
        self.scene_graph.parents(node)
    }

    pub fn sibling_nodes<'a>(&'a self, node: NodeId) -> SiblingNodeIter<'a> {
        self.scene_graph.siblings(node)
    }

    pub fn direct_children_nodes<'a>(&'a self, node: NodeId) -> DirectChildrenNodeIter<'a> {
        self.scene_graph.direct_children(node)
    }

    pub fn all_children_nodes<'a>(&'a self, node: NodeId) -> AllChildrenNodeIter<'a> {
        self.scene_graph.all_children(node)
    }

    pub fn set_node_local_transform(
        &mut self,
        node: NodeId,
        translation: impl Into<Option<Vec3>>,
        rotation: impl Into<Option<Quat>>,
        scale: impl Into<Option<Vec3>>,
    ) {
        self.scene_graph
            .set_local_transform(node, translation, rotation, scale);
    }

    pub fn mesh<'a>(&'a self, node: NodeId) -> Option<&'a MeshInstance> {
        self.meshes.get(node)
    }

    pub fn set_mesh_weights(&mut self, node: NodeId, weights: &[f32]) {
        self.meshes.set_weights(node, weights);
    }

    /// Returns the camera associated with the given node, if it exists.
    pub fn camera<'a>(&'a self, node: NodeId) -> Option<&'a Camera> {
        self.cameras.get(node)
    }

    /// Sets the camera associated with the given node. If a camera already exists for that node, it will be replaced and the old camera will be returned.
    pub fn set_camera(&mut self, node: NodeId, camera: Camera) -> Option<Camera> {
        self.cameras.set(node, camera)
    }

    pub fn remove(&mut self, node: NodeId) -> Option<Node> {
        if !self.contains(node) {
            return None;
        }
        for (id, _) in self.scene_graph.all_children(node) {
            self.meshes.remove(id);
        }
        self.meshes.remove(node);
        self.scene_graph.remove(node)
    }
}

#[derive(Debug)]
pub struct Renderer {
    width: u32,
    height: u32,
    ctx: Context,
    primitive: PrimitiveRenderer,
    bloom: post_processing::Bloom,
    tone_mapping: post_processing::ToneMapping,
}

impl Renderer {
    pub fn new(width: u32, height: u32, format: wgpu::TextureFormat, ctx: Context) -> Self {
        let primitive = PrimitiveRenderer::new(&ctx);
        let bloom = post_processing::Bloom::new(width, height, 5, 0.02, &ctx);
        let tone_mapping = post_processing::ToneMapping::new(width, height, format, &ctx);

        Renderer {
            width,
            height,
            ctx,
            primitive,
            bloom,
            tone_mapping,
        }
    }

    /// Returns the width of the renderer output in pixels. This needs to match the width of the render target in [`Renderer::render()`] or [`PreparedScene::render()`].
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Returns the height of the renderer output in pixels. This needs to match the height of the render target in [`Renderer::render()`] or [`PreparedScene::render()`].
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Changes the size of the renderer output. The new width and height must be non-zero.
    ///
    /// This function needs be called before rendering to a target with different dimensions.
    ///
    /// # Panics
    /// This function will panic if the new width or height is zero.
    pub fn resize(&mut self, width: u32, height: u32) {
        // resizing is expensive
        if self.width == width && self.height == height {
            return;
        }
        if width == 0 || height == 0 {
            panic!(
                "Renderer dimensions must be non-zero, got width: {}, height: {}",
                width, height
            );
        }
        self.width = width;
        self.height = height;
        self.bloom.resize(width, height, &self.ctx);
        self.tone_mapping.resize(width, height, &self.ctx);
    }

    /// Renders the scene to the given target from the `camera` point of view.
    ///
    /// # Panics
    /// This function will panic if:
    /// - the renderer and the target have different dimensions;
    /// - the renderer and the target have different texture format;
    /// - the scene and the render are using different [Context]s.
    pub fn render(&mut self, scene: &Scene, camera: NodeId, target: &wgpu::TextureView) {
        assert!(self.ctx == scene.ctx);

        let _ = camera;
        let _ = target;
        let _ = scene.meshes;
        let _ = scene.scene_graph;

        let mut encoder = self
            .ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("mogra render command encoder"),
            });

        let _prepared_primitives =
            self.primitive
                .prepare(scene.meshes.meshes(), &self.ctx, &mut encoder);

        let _ = self.bloom.input_texture_view();
        self.bloom
            .render(self.tone_mapping.input_texture_view(), &mut encoder);
        self.tone_mapping.render(target, &mut encoder);

        self.ctx.queue.submit([encoder.finish()]);
    }

    pub fn context(&self) -> &Context {
        &self.ctx
    }
}

impl Clone for Renderer {
    fn clone(&self) -> Self {
        // should create a deep copy
        todo!()
    }
}
