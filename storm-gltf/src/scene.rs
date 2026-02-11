use std::path::Path;

use anyhow::{Context, Result};
use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};
use storm::{
    mesh::MeshInstanceBuilder,
    scene_graph::{NodeBuilder, SceneGraph},
};
use storm_animation::AnimationManager;

use crate::Mesh;

/// A node in the node hierarchy. When the node contains [skin](Node::skin),
/// all [mesh.primitives](Mesh::primitives) **MUST** contain [JOINTS_0](PrimitiveAttributes::joints_0)
/// and [WEIGHTS_0](PrimitiveAttributes::weights_0) attributes.
/// A node **MAY** have either a `matrix` or any combination of
/// [translation](Node::translation)/[rotation](Node::rotation)/[scale](Node::scale)
/// (TRS) properties. TRS properties are converted to matrices and postmultiplied in the
/// `T * R * S` order to compose the transformation matrix; first the scale is applied to
/// the vertices, then the rotation, and then the translation. If none are provided, the
/// transform is the identity. When a node is targeted for animation (referenced by an
/// [animation.channel.target](AnimationChannel::target)), [matrix](Node::matrix)
/// **MUST NOT** be present.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Node {
    /// [NodeId][crate::node::NodeId], if the resource has been loaded. Cleared once the scene has been loaded.
    #[serde(skip)]
    id: Option<storm::scene_graph::NodeId>,

    /// The index of the camera referenced by this node.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    camera: Option<usize>,

    /// The indices of this node's children.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    children: Vec<usize>,

    /// The index of the skin referenced by this node.
    /// When a skin is referenced by a node within a scene,
    /// all joints used by the skin **MUST** belong to the same scene.
    /// When defined, [mesh](Node::mesh) **MUST** also be defined.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    skin: Option<usize>,

    /// A floating-point 4x4 transformation matrix stored in column-major order.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    matrix: Option<[f32; 16]>,

    /// The index of the mesh in this node.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    mesh: Option<usize>,

    /// The node’s unit quaternion rotation in the order (x, y, z, w), where w is the scalar.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    rotation: Option<[f32; 4]>,

    /// The node’s non-uniform scale, given as the scaling factors along the x, y, and z axes.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    scale: Option<[f32; 3]>,

    /// The node’s translation along the x, y, and z axes.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    translation: Option<[f32; 3]>,

    /// The weights of the instantiated morph target. The number of array elements
    /// **MUST** match the number of morph targets of the referenced mesh.
    /// When defined, [mesh](Node::mesh) **MUST** also be defined.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    weights: Option<Vec<f32>>,

    /// The user-defined name of this object. This is not necessarily unique, e.g.,
    /// an accessor and a buffer could have the same name, or two accessors could
    /// even have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

impl Node {
    /// Storm storage id, if the resource has been loaded. Cleared once the scene has been loaded.
    pub(super) fn id(&self) -> Option<storm::scene_graph::NodeId> {
        self.id
    }

    fn load(
        index: usize,
        nodes: &mut [Node],
        parent: Option<storm::scene_graph::NodeId>,
        scene_graph: &mut SceneGraph,
    ) -> Result<storm::scene_graph::NodeId> {
        let node = nodes
            .get_mut(index)
            .with_context(|| format!("node {index} is out of range."))?;

        let node_ctx = || format!("failed to load node {index}.");

        let name = node.name.clone().unwrap_or_default();
        let mut builder = NodeBuilder::default().name(name);
        if let Some(parent) = parent {
            builder = builder.parent(parent);
        }
        match &node.matrix {
            Some(matrix) => {
                let (scale, rotation, translation) =
                    Mat4::from_cols_array(matrix).to_scale_rotation_translation();
                builder = builder
                    .local_scale(scale)
                    .local_rotation(rotation)
                    .local_translation(translation)
            }
            None => {
                if let Some(scale) = node.scale {
                    builder = builder.local_scale(scale);
                }
                if let Some(rotation) = node.rotation {
                    builder = builder.local_rotation(Quat::from_array(rotation));
                }
                if let Some(translation) = node.translation {
                    builder = builder.local_translation(translation);
                }
            }
        };
        let id = builder.build(scene_graph)?;
        node.id = Some(id);

        for child_index in node.children.clone() {
            Self::load(child_index, nodes, Some(id), scene_graph).with_context(node_ctx)?;
        }

        Ok(id)
    }

    fn load_mesh(
        index: usize,
        nodes: &[Node],
        meshes: &mut [Mesh],
        base_path: &Path,
        accessors: &[super::Accessor],
        materials: &mut [super::Material],
        default_material: &mut Option<storm::mesh::material::Material>,
        textures: &mut [super::Texture],
        samplers: &mut [super::Sampler],
        images: &mut [super::Image],
        buffer_views: &[super::BufferView],
        buffers: &[super::Buffer],
        ctx: &storm::Context,
        encoder: &mut wgpu::CommandEncoder,
        scene: &mut storm::Scene,
    ) -> Result<()> {
        let node = &nodes[index];
        let node_ctx = || format!("failed to load node {index}.");

        if let Some(mesh_index) = node.mesh {
            let gltf_mesh = meshes
                .get_mut(mesh_index)
                .with_context(|| format!("node.mesh {mesh_index} is out of range."))
                .with_context(node_ctx)?;
            let storm_mesh = gltf_mesh
                .load(
                    base_path,
                    accessors,
                    materials,
                    default_material,
                    textures,
                    samplers,
                    images,
                    buffer_views,
                    buffers,
                    ctx,
                    encoder,
                )
                .with_context(|| format!("Failed to load mesh {mesh_index}."))
                .with_context(node_ctx)?;
            let mut builder = MeshInstanceBuilder::new(storm_mesh).node(node.id.unwrap());
            if let Some(weights) = &node.weights {
                builder = builder.weights(weights.clone());
            } else if let Some(weights) = gltf_mesh.weights() {
                builder = builder.weights(weights.clone());
            }
            builder.build(scene).unwrap();
        }

        for &child_index in node.children.iter() {
            Self::load_mesh(
                child_index,
                nodes,
                meshes,
                base_path,
                accessors,
                materials,
                default_material,
                textures,
                samplers,
                images,
                buffer_views,
                buffers,
                ctx,
                encoder,
                scene,
            )?;
        }

        Ok(())
    }
}

/// The root nodes of a scene.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Scene {
    /// The indices of each root node.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    nodes: Vec<usize>,

    /// The user-defined name of this object. This is not necessarily unique, e.g.,
    /// an accessor and a buffer could have the same name, or two accessors could
    /// even have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

impl Scene {
    /// The user-defined name of this object. This is not necessarily unique, e.g.,
    /// an accessor and a buffer could have the same name, or two accessors could
    /// even have the same name.
    pub(super) fn name(&self) -> &Option<String> {
        &self.name
    }
}

// Joints and matrices defining a skin.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Skin {
    /// Nodes using this skin. Cleared once the scene has been loaded.
    #[serde(skip)]
    nodes: Vec<storm::scene_graph::NodeId>,

    /// The index of the accessor containing the floating-point 4x4 inverse-bind matrices.
    /// Its [accessor.count](Accessor::count) property **MUST** be greater than or equal to
    /// the number of elements of the joints array. When undefined, each matrix is a 4x4
    /// identity matrix.
    #[serde(rename = "inverseBindMatrices")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    inverse_bind_matrices: Option<usize>,

    /// The index of the node used as a skeleton root. The node **MUST** be the closest common
    /// root of the joints hierarchy or a direct or indirect parent node of the closest common root.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    skeleton: Option<usize>,

    /// Indices of skeleton nodes, used as joints in this skin.
    joints: Vec<usize>,

    /// The user-defined name of this object. This is not necessarily unique, e.g.,
    /// an accessor and a buffer could have the same name, or two accessors could
    /// even have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

impl super::GltfAsset {
    pub fn load_scene_into(
        &mut self,
        scene_index: usize,
        base_node: Option<storm::scene_graph::NodeId>,
        scene: &mut storm::Scene,
        animation_manager: &mut AnimationManager,
        ctx: &storm::Context,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Result<Vec<storm::scene_graph::NodeId>> {
        let root_nodes_idx = self
            .json
            .scenes
            .get(scene_index)
            .with_context(|| format!("scene {scene_index} is out of range."))?
            .nodes
            .clone();

        let scene_ctx = || format!("Failed to load scene {scene_index}.");

        let mut root_nodes_ids = Vec::with_capacity(root_nodes_idx.len());
        for &node_index in root_nodes_idx.iter() {
            root_nodes_ids.push(
                Node::load(
                    node_index,
                    &mut self.json.nodes,
                    base_node,
                    &mut scene.scene_graph,
                )
                .with_context(scene_ctx)?,
            );
        }

        for node_index in root_nodes_idx {
            Node::load_mesh(
                node_index,
                &self.json.nodes,
                &mut self.json.meshes,
                &self.base_path,
                &self.json.accessors,
                &mut self.json.materials,
                &mut self.default_material,
                &mut self.json.textures,
                &mut self.json.samplers,
                &mut self.json.images,
                &self.json.buffer_views,
                &self.json.buffers,
                ctx,
                encoder,
                scene,
            )
            .with_context(scene_ctx)?;
        }

        // let mut data = Vec::with_capacity(self.json.skins.len());
        // for (skin_idx, skin) in &mut self.json.skins.iter_mut().enumerate() {
        //     let skin_ctx = move || format!("Failed to skin {skin_idx}.");
        //     if !skin.nodes.is_empty() {
        //         let mut joints = Vec::with_capacity(skin.joints.len());
        //         for (joint_idx, &index) in skin.joints.iter().enumerate() {
        //             joints.push(
        //                 self.json
        //                     .nodes
        //                     .get(index)
        //                     .with_context(|| {
        //                         format!("skin.nodes[{joint_idx}] {index} is out of range.")
        //                     })
        //                     .with_context(skin_ctx)
        //                     .with_context(scene_ctx)?
        //                     .id
        //                     .with_context(|| {
        //                         format!("skin.nodes[{joint_idx}] {index} is not part of scene.")
        //                     })
        //                     .with_context(skin_ctx)
        //                     .with_context(scene_ctx)?,
        //             );
        //         }
        //         data.push((
        //             std::mem::take(&mut skin.nodes),
        //             joints,
        //             skin.inverse_bind_matrices,
        //             skin_ctx,
        //         ));
        //     }
        // }

        // for (nodes, joints, inverse_bind_matrices, skin_ctx) in data {
        //     let mut builder = SkinBuilder::default().nodes(joints);
        //     if let Some(inverse_bind_matrices) = inverse_bind_matrices {
        //         struct RegisterBindMatrices {
        //             builder: SkinBuilder,
        //         }

        //         impl IteratorConsumer<'_, Mat4> for RegisterBindMatrices {
        //             type Return = SkinBuilder;

        //             fn consume<I: Iterator<Item = Mat4>>(self, iter: I) -> Result<Self::Return> {
        //                 Ok(self.builder.inverse_bind_matrices(iter))
        //             }
        //         }

        //         let consumer = RegisterBindMatrices { builder };
        //         let accessor = self
        //             .json
        //             .accessors
        //             .get(inverse_bind_matrices)
        //             .with_context(|| {
        //                 format!("inverse_bind_matrices {inverse_bind_matrices} is out of range")
        //             })
        //             .with_context(skin_ctx)
        //             .with_context(scene_ctx)?;

        //         builder =
        //             accessor.iter_mat4(&self.json.buffer_views, &self.json.buffers, consumer)?;
        //     }

        //     let skin = builder.build(scene).unwrap();
        //     for node in nodes {
        //         // scene.add_skin_to_node(skin, node);
        //         todo!("add skin to node");
        //     }
        // }

        for animation in &self.json.animations {
            let id = animation_manager.insert(animation.load(
                &self.json.nodes,
                &self.json.accessors,
                &self.json.buffer_views,
                &self.json.buffers,
            )?);
            animation_manager.start(id).unwrap();
        }

        for node in &mut self.json.nodes {
            node.id = None;
        }

        Ok(root_nodes_ids)
    }

    fn load_node(
        &mut self,
        index: usize,
        parent: Option<storm::scene_graph::NodeId>,
        scene: &mut storm::Scene,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<storm::scene_graph::NodeId> {
        let node = self
            .json
            .nodes
            .get(index)
            .with_context(|| format!("node {index} is out of range."))?;

        let children = node.children.clone();
        let node_ctx = || format!("Failed to load node {index}.");

        let mut builder = NodeBuilder::default().name(node.name.clone().unwrap_or("".to_string()));
        if let Some(parent) = parent {
            builder = builder.parent(parent);
        }
        builder = match &node.matrix {
            Some(matrix) => {
                let (scale, rotation, translation) =
                    Mat4::from_cols_array(matrix).to_scale_rotation_translation();
                builder
                    .local_scale(scale)
                    .local_rotation(rotation)
                    .local_translation(translation)
            }
            None => builder
                .local_scale(node.scale.map_or(Vec3::ZERO, Vec3::from_array))
                .local_rotation(node.rotation.map_or(Quat::IDENTITY, Quat::from_array))
                .local_translation(node.translation.map_or(Vec3::ONE, Vec3::from_array)),
        };
        let id = builder
            .build(&mut scene.scene_graph)
            .with_context(node_ctx)?;

        if let Some(mesh_index) = node.mesh {
            let gltf_mesh = self
                .json
                .meshes
                .get_mut(mesh_index)
                .with_context(|| format!("node.mesh {mesh_index} is out of range."))
                .with_context(node_ctx)?;
            let storm_mesh = gltf_mesh
                .load(
                    &self.base_path,
                    &self.json.accessors,
                    &mut self.json.materials,
                    &mut self.default_material,
                    &mut self.json.textures,
                    &mut self.json.samplers,
                    &mut self.json.images,
                    &self.json.buffer_views,
                    &self.json.buffers,
                    scene.context(),
                    encoder,
                )
                .with_context(|| format!("Failed to load mesh {mesh_index}."))
                .with_context(node_ctx)?;

            let mut builder = MeshInstanceBuilder::new(storm_mesh).node(id);
            if let Some(weights) = &node.weights {
                builder = builder.weights(weights.clone());
            } else if let Some(weights) = gltf_mesh.weights() {
                builder = builder.weights(weights.clone());
            }
            builder.build(scene).unwrap();
        }

        // node.id = Some(id);
        if let Some(index) = node.skin {
            self.json
                .skins
                .get_mut(index)
                .with_context(|| format!("node.skin {index} is out of range."))
                .with_context(node_ctx)?
                .nodes
                .push(id);
        }

        for child in children {
            self.load_node(child, Some(id), scene, encoder)?;
        }

        Ok(id)
    }
}
