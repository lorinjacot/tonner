use std::{collections::HashMap, path::Path};

use anyhow::{Context, Result, anyhow};
use glam::{Mat4, Quat};
use serde::{Deserialize, Serialize};
use tonner::{
    geometry::skin::SkinManager,
    mesh::{MeshInstance, MeshInstanceId},
    scene_graph::{NodeBuilder, SceneGraph},
};

use crate::{Mesh, skin::Skin};

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
    pub(super) id: Option<tonner::scene_graph::NodeId>,

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
    pub(super) fn load(
        index: usize,
        nodes: &mut [Node],
        parent: Option<tonner::scene_graph::NodeId>,
        scene_graph: &mut SceneGraph,
    ) -> Result<tonner::scene_graph::NodeId> {
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

    pub(super) fn load_mesh(
        index: usize,
        nodes: &[Node],
        skins: &mut [Skin],
        meshes: &mut [Mesh],
        base_path: &Path,
        accessors: &[super::Accessor],
        materials: &mut [super::Material],
        default_material: &mut Option<tonner::mesh::material::Material>,
        textures: &mut [super::Texture],
        samplers: &mut [super::Sampler],
        images: &mut [super::Image],
        buffer_views: &[super::BufferView],
        buffers: &[super::Buffer],
        mesh_instances: &mut HashMap<MeshInstanceId, MeshInstance>,
        skin_manager: &mut SkinManager,
        ctx: &tonner::Context,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Result<()> {
        let node = &nodes[index];
        let node_ctx = || format!("failed to load node {index}.");

        let skin = if let Some(skin_index) = node.skin {
            if node.mesh.is_none() {
                return Err(anyhow!(
                    "A node can only have a skin if it also has a mesh."
                ))
                .with_context(node_ctx);
            }

            let skin = skins
                .get_mut(skin_index)
                .with_context(|| format!("node.skin {skin_index} is out of range."))
                .with_context(node_ctx)?;

            Some(skin.load(nodes, accessors, buffer_views, buffers, skin_manager)?)
        } else {
            None
        };

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
            let mut instance = match skin {
                Some(skin) => storm_mesh.new_instance_with_skin(node.id.unwrap(), skin),
                None => storm_mesh.new_instance(node.id.unwrap()),
            };
            if let Some(weights) = &node.weights {
                instance.set_weights(weights);
            } else if let Some(weights) = gltf_mesh.weights() {
                instance.set_weights(weights);
            }
            mesh_instances.insert(instance.id(), instance);
        }

        for &child_index in node.children.iter() {
            Self::load_mesh(
                child_index,
                nodes,
                skins,
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
                mesh_instances,
                skin_manager,
                ctx,
                encoder,
            )?;
        }

        Ok(())
    }
}
