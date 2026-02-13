use std::{collections::HashMap, fmt::Display, ops::Range};

use bytemuck::{Pod, Zeroable, cast_slice};
use glam::{Mat4, Vec4};
use uuid::{NonNilUuid, Uuid};

use crate::{
    Context,
    geometry::{GeometryIndices, MAX_MORPH_TARGET_COUNT},
    mesh::{
        Mesh,
        asset::{MeshPrimitive, MeshPrimitiveId},
        material::AlphaMode,
    },
    renderer::RenderError,
    scene_graph::{NodeId, SceneGraph},
    skin::{PreparedSkins, SkinId},
};

/// A unique id for a [mesh instance][MeshInstance]. A mesh instance will always have the same id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeshInstanceId {
    uuid: NonNilUuid,
}

impl Display for MeshInstanceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MeshInstanceId({})", self.uuid)
    }
}

/// A mesh instance is a [Mesh] associated with a scene graph node.
/// The associated node global transform is used to get the position,
/// orientation and scale of the mesh.
/// This struct also stores the morph target weights and an optional skeleton/skin, if supported
/// by the mesh.
/// To create a new instance, see [Mesh::new_instance()].
#[derive(Debug)]
pub struct MeshInstance {
    id: MeshInstanceId,
    pub name: String,
    mesh: Mesh,
    pub node: NodeId,
    weights: [f32; MAX_MORPH_TARGET_COUNT],
    skin: Option<SkinId>,
}

impl MeshInstance {
    /// Unique identifier for the mesh. This will always return the same value.
    pub fn id(&self) -> MeshInstanceId {
        self.id
    }

    /// Mesh instantiated by this struct. This will always return the same value.
    pub fn mesh(&self) -> &Mesh {
        &self.mesh
    }

    /// Morph target weights. Used to deform the mesh. The length of
    /// the returned slice will always match [`Mesh::morph_target_count()`].
    pub fn weights(&self) -> &[f32] {
        &self.weights
    }

    /// Sets the morph target weights. The length of `weights` must match
    /// [`Mesh::morph_target_count()`].
    ///
    /// ## Panics
    /// This function will panic if the length of `weights` differs from [`Mesh::morph_target_count()`].
    pub fn set_weights(&mut self, weights: &[f32]) {
        self.weights[0..self.mesh.morph_target_count()].copy_from_slice(weights);
    }

    /// If supported by the mesh, this skin/skeleton will be applied
    /// to it.
    pub fn skin(&self) -> Option<SkinId> {
        self.skin
    }
}

/// Methods to create a new instance.
impl Mesh {
    /// Create a mesh instance with no skin/skeleton.
    /// By default, the name is an empty string and
    /// all [morph target weight][MeshInstance::weights()] are `0.0`.
    pub fn new_instance(&self, node: NodeId) -> MeshInstance {
        MeshInstance {
            id: MeshInstanceId {
                uuid: NonNilUuid::new(Uuid::new_v4()).unwrap(),
            },
            name: String::new(),
            mesh: self.clone(),
            node,
            weights: [0.0; _],
            skin: None,
        }
    }

    /// Same as [Self::new_instance()] but with a skin/skeleton.
    pub fn new_instance_with_skin(&self, node: NodeId, skin: SkinId) -> MeshInstance {
        MeshInstance {
            id: MeshInstanceId {
                uuid: NonNilUuid::new(Uuid::new_v4()).unwrap(),
            },
            name: String::new(),
            mesh: self.clone(),
            node,
            weights: [0.0; _],
            skin: Some(skin),
        }
    }
}

pub(crate) struct PrimitiveRenderer {
    vertex_buffer: wgpu::Buffer,
}

impl PrimitiveRenderer {
    pub(crate) fn new(ctx: &Context) -> Self {
        let vertex_buffer = Self::create_vertex_buffer(0, false, ctx.device());

        Self { vertex_buffer }
    }

    pub(crate) fn prepare<'a, 'b>(
        &'a mut self,
        mesh_instances: impl IntoIterator<Item = &'b MeshInstance>,
        scene_graph: &SceneGraph,
        prepared_skins: PreparedSkins,
        ctx: &Context,
    ) -> Result<PreparedPrimitives<'a>, RenderError> {
        let mut opaque_primitives = PrimitivesByPipeline(HashMap::new());
        let mut transparent_primitives = PrimitivesByPipeline(HashMap::new());

        let mut primitive_count = 0;
        for mesh_instance in mesh_instances.into_iter() {
            let model_matrix = scene_graph
                .get(mesh_instance.node)
                .ok_or(RenderError::InvalidMeshInstanceNode(
                    mesh_instance.id,
                    mesh_instance.node,
                ))?
                .global_transformation();
            let pipeline_index = model_matrix.determinant().is_sign_negative() as usize;
            let joint_offset = match mesh_instance.skin {
                Some(skin) => prepared_skins
                    .offset(skin)
                    .ok_or(RenderError::InvalidMeshInstanceSkin(mesh_instance.id, skin))?,
                None => 0,
            };
            let data = PrimitiveInstanceVertex {
                model_matrix,
                weights_0: Vec4::from_slice(&mesh_instance.weights[0..4]),
                weights_1: Vec4::from_slice(&mesh_instance.weights[4..8]),
                joint_offset,
                _pad: [0; 3],
            };

            for primitive in mesh_instance.mesh.primitives() {
                primitive_count += 1;
                let primitves = match primitive.alpha_mode() {
                    AlphaMode::Opaque | AlphaMode::Mask => &mut opaque_primitives,
                    AlphaMode::Blend => &mut transparent_primitives,
                };
                let pipeline = primitive.render_pipelines()[pipeline_index].clone();
                primitves
                    .0
                    .entry(pipeline)
                    .or_default()
                    .entry(primitive.id())
                    .or_insert_with(|| {
                        (
                            primitive.clone(),
                            PrimitiveInstances {
                                count: 0,
                                bounds: 0..0,
                                data: Vec::with_capacity(1),
                            },
                        )
                    })
                    .1
                    .data
                    .push(data);
            }
        }

        let mut data = Vec::with_capacity(primitive_count);
        let size = size_of::<PrimitiveInstanceVertex>();
        opaque_primitives
            .0
            .values_mut()
            .chain(transparent_primitives.0.values_mut())
            .for_each(|primitives| {
                for (_, instances) in primitives.values_mut() {
                    instances.count = instances.data.len() as u32;
                    let start = data.len() * size;
                    data.append(&mut instances.data);
                    let end = data.len() * size;
                    instances.bounds = start as u64..end as u64;
                }
            });

        let size = data.len() * size_of::<PrimitiveInstanceVertex>();
        let aligned_size = wgpu::util::align_to(size as u64, wgpu::COPY_BUFFER_ALIGNMENT);
        if self.vertex_buffer.size() < size as u64 {
            self.vertex_buffer = Self::create_vertex_buffer(aligned_size, true, ctx.device());
            let mut view = self.vertex_buffer.get_mapped_range_mut(..);
            view[..size].copy_from_slice(cast_slice(&data));
            drop(view);
            self.vertex_buffer.unmap();
        } else {
            ctx.queue()
                .write_buffer(&self.vertex_buffer, 0, cast_slice(&data));
        }

        Ok(PreparedPrimitives {
            vertex_buffer: &self.vertex_buffer,
            opaque_primitives,
            transparent_primitives,
        })
    }

    fn create_vertex_buffer(
        size: u64,
        mapped_at_creation: bool,
        device: &wgpu::Device,
    ) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Mesh instance vertex buffer"),
            size,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation,
        })
    }
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub(crate) struct PrimitiveInstanceVertex {
    model_matrix: Mat4,
    weights_0: Vec4,
    weights_1: Vec4,
    joint_offset: u32,
    _pad: [u32; 3],
}

#[derive(Debug)]
struct PrimitiveInstances {
    count: u32,
    data: Vec<PrimitiveInstanceVertex>,
    bounds: Range<u64>,
}

#[derive(Debug)]
struct PrimitivesByPipeline(
    HashMap<wgpu::RenderPipeline, HashMap<MeshPrimitiveId, (MeshPrimitive, PrimitiveInstances)>>,
);

impl PrimitivesByPipeline {
    fn render(&self, vertex_buffer: &wgpu::Buffer, render_pass: &mut wgpu::RenderPass) {
        for (pipeline, primitives) in &self.0 {
            render_pass.set_pipeline(pipeline);

            for (primitive, instances) in primitives.values() {
                render_pass.set_vertex_buffer(0, vertex_buffer.slice(instances.bounds.clone()));
                render_pass.set_bind_group(1, primitive.bind_group(), &[]);
                let instances = 0..instances.count;
                match primitive.indices() {
                    Some(GeometryIndices {
                        buffer,
                        format,
                        count,
                    }) => {
                        render_pass.set_index_buffer(buffer.slice(..), *format);
                        render_pass.draw_indexed(0..*count as u32, 0, instances);
                    }
                    None => {
                        render_pass.draw(0..primitive.vertex_count() as u32, instances);
                    }
                }
            }
        }
    }
}

pub(crate) struct PreparedPrimitives<'a> {
    vertex_buffer: &'a wgpu::Buffer,
    opaque_primitives: PrimitivesByPipeline,
    transparent_primitives: PrimitivesByPipeline,
}

impl<'a> PreparedPrimitives<'a> {
    pub(crate) fn render_opaque_primitives(&mut self, opaque_render_pass: &mut wgpu::RenderPass) {
        self.opaque_primitives
            .render(self.vertex_buffer, opaque_render_pass);
    }

    pub(crate) fn render_transparent_primitives(&mut self, transparent_render_pass: &mut wgpu::RenderPass) {
        self.transparent_primitives
            .render(self.vertex_buffer, transparent_render_pass);
    }
}
