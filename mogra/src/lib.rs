use crate::{
    buffer::WritableBuffer,
    camera::{Camera, CameraManager, CameraUniform},
    light::{LightContext, LightManager, create_point_light_buffer},
    mesh::{MeshInstance, MeshManager},
    primitive::PrimitiveRenderer,
    scene_graph::{SceneGraph, SiblingNodeIter},
};
use bytemuck::bytes_of;
use glam::{Mat4, Quat, Vec3};
pub use node::NodeBuilder;
pub use scene_graph::{
    AllChildrenNodeIter, DirectChildrenNodeIter, Node, NodeId, NodeIter, ParentNodeIter,
    RootNodeIter,
};

mod buffer;
pub mod camera;
pub mod geometry;
pub mod light;
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
    light: LightContext,
    render_bind_group_layout: wgpu::BindGroupLayout,
    bloom: post_processing::BloomContext,
    tone_mapping: post_processing::ToneMappingContext,
}

impl Context {
    /// Create the context using the provided `device` and `queue`. All GPU resourcces and
    /// GPU operations will be created on them.
    pub fn from_device(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mogra context command encoder"),
        });

        let light = LightContext::new(&device, &mut encoder);
        let render_bind_group_layout = create_render_bind_group_layout(&device);
        let bloom = post_processing::BloomContext::new(&device);
        let tone_mapping = post_processing::ToneMappingContext::new(&device);

        queue.submit([encoder.finish()]);

        Context {
            device,
            queue,
            light,
            render_bind_group_layout,
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
    lights: LightManager,
}

impl Scene {
    pub fn new(ctx: Context) -> Self {
        let scene_graph = SceneGraph::new();
        let meshes = MeshManager::new(&ctx);
        let cameras = CameraManager::new();
        let lights = LightManager::new();

        Scene {
            ctx,
            scene_graph,
            meshes,
            cameras,
            lights,
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
    /// - the scene and the render are using different [Context]s;
    /// - the camera is not part of the scene.
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
    render_bind_group_layout: wgpu::BindGroupLayout,
    dummy_joint_buffer: wgpu::Buffer,
    camera_buffer: wgpu::Buffer,
    point_light_buffer: WritableBuffer,
    primitive: PrimitiveRenderer,
    bloom: post_processing::Bloom,
    tone_mapping: post_processing::ToneMapping,
}

impl Renderer {
    pub fn new(width: u32, height: u32, format: wgpu::TextureFormat, ctx: Context) -> Self {
        let render_bind_group_layout = ctx.render_bind_group_layout.clone();
        let point_light_buffer = create_point_light_buffer(&ctx.device);
        let primitive = PrimitiveRenderer::new(width, height, &ctx);
        let bloom = post_processing::Bloom::new(width, height, 5, 0.02, &ctx);
        let tone_mapping = post_processing::ToneMapping::new(width, height, format, &ctx);
        let dummy_joint_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mogra dummy joint buffer"),
            size: size_of::<Mat4>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let camera_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mogra camera buffer"),
            size: size_of::<CameraUniform>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Renderer {
            width,
            height,
            ctx,
            render_bind_group_layout,
            dummy_joint_buffer,
            camera_buffer,
            point_light_buffer,
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
        self.primitive.resize(width, height, &self.ctx);
        self.bloom.resize(width, height, &self.ctx);
        self.tone_mapping.resize(width, height, &self.ctx);
    }

    /// Renders the scene to the given target from the `camera` point of view.
    ///
    /// # Panics
    /// This function will panic if:
    /// - the renderer and the target have different dimensions;
    /// - the renderer and the target have different texture format;
    /// - the scene and the render are using different [Context]s;
    /// - the camera is not part of the scene.
    pub fn render(&mut self, scene: &Scene, camera: NodeId, target: &wgpu::TextureView) {
        assert!(self.ctx == scene.ctx);

        let mut encoder = self
            .ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("mogra render command encoder"),
            });

        let projection = scene
            .cameras
            .get(camera)
            .expect("scene should contain the camera")
            .projection_matrix();
        let camera_transform = scene.scene_graph[camera].global_transform();
        let camera_position = camera_transform.w_axis.truncate();
        let view = Mat4::look_at_rh(
            camera_position,
            camera_transform.transform_vector3(-Vec3::Z),
            camera_transform.transform_vector3(Vec3::Y),
        );
        let camera_uniform = CameraUniform {
            view_projection: projection * view,
            view,
            projection_inverse: projection.inverse(),
            position: camera_position,
            _pad: 0,
        };
        self.ctx
            .queue
            .write_buffer(&self.camera_buffer, 0, bytes_of(&camera_uniform));

        scene.lights.write_point_light(
            &scene.scene_graph,
            &mut self.point_light_buffer,
            &self.ctx,
            &mut encoder,
        );

        let _render_bind_group = self
            .ctx
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("mogra render bind group"),
                layout: &self.render_bind_group_layout,
                entries: &[
                    // skins
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.dummy_joint_buffer.as_entire_binding(),
                    },
                    // camera
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.camera_buffer.as_entire_binding(),
                    },
                    // lights
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: self.point_light_buffer.buffer().as_entire_binding(),
                    },
                    // // irradiance map
                    // wgpu::BindGroupEntry {
                    //     binding: 4,
                    //     resource: wgpu::BindingResource::TextureView(
                    //         environment.irradiance_map_view(),
                    //     ),
                    // },
                    // wgpu::BindGroupEntry {
                    //     binding: 5,
                    //     resource: wgpu::BindingResource::Sampler(
                    //         environment.irradiance_map_sampler(),
                    //     ),
                    // },
                    // // prefilter map
                    // wgpu::BindGroupEntry {
                    //     binding: 6,
                    //     resource: wgpu::BindingResource::TextureView(
                    //         environment.prefilter_map_view(),
                    //     ),
                    // },
                    // wgpu::BindGroupEntry {
                    //     binding: 7,
                    //     resource: wgpu::BindingResource::Sampler(
                    //         environment.prefilter_map_sampler(),
                    //     ),
                    // },
                    // // BRDF LUT
                    // wgpu::BindGroupEntry {
                    //     binding: 8,
                    //     resource: wgpu::BindingResource::TextureView(environment.brdf_lut_view()),
                    // },
                    // wgpu::BindGroupEntry {
                    //     binding: 9,
                    //     resource: wgpu::BindingResource::Sampler(environment.brdf_lut_sampler()),
                    // },
                ],
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

fn create_render_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("render bind group layout"),
        entries: &[
            // skins
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // camera
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // lights
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // // irradiance map
            // wgpu::BindGroupLayoutEntry {
            //     binding: 4,
            //     visibility: wgpu::ShaderStages::FRAGMENT,
            //     ty: wgpu::BindingType::Texture {
            //         sample_type: wgpu::TextureSampleType::Float { filterable: true },
            //         view_dimension: wgpu::TextureViewDimension::Cube,
            //         multisampled: false,
            //     },
            //     count: None,
            // },
            // wgpu::BindGroupLayoutEntry {
            //     binding: 5,
            //     visibility: wgpu::ShaderStages::FRAGMENT,
            //     ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            //     count: None,
            // },
            // // prefilter map
            // wgpu::BindGroupLayoutEntry {
            //     binding: 6,
            //     visibility: wgpu::ShaderStages::FRAGMENT,
            //     ty: wgpu::BindingType::Texture {
            //         sample_type: wgpu::TextureSampleType::Float { filterable: true },
            //         view_dimension: wgpu::TextureViewDimension::Cube,
            //         multisampled: false,
            //     },
            //     count: None,
            // },
            // wgpu::BindGroupLayoutEntry {
            //     binding: 7,
            //     visibility: wgpu::ShaderStages::FRAGMENT,
            //     ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            //     count: None,
            // },
            // // BRDF LUT
            // wgpu::BindGroupLayoutEntry {
            //     binding: 8,
            //     visibility: wgpu::ShaderStages::FRAGMENT,
            //     ty: wgpu::BindingType::Texture {
            //         sample_type: wgpu::TextureSampleType::Float { filterable: true },
            //         view_dimension: wgpu::TextureViewDimension::D2,
            //         multisampled: false,
            //     },
            //     count: None,
            // },
            // wgpu::BindGroupLayoutEntry {
            //     binding: 9,
            //     visibility: wgpu::ShaderStages::FRAGMENT,
            //     ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            //     count: None,
            // },
        ],
    })
}
