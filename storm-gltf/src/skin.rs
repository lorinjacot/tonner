use anyhow::{Context, Result};
use glam::Mat4;
use serde::{Deserialize, Serialize};
use tonner::geometry::skin::{SkinBuilder, SkinId, SkinManager};

use crate::{Accessor, Buffer, BufferView, accessor::IteratorConsumer, node::Node};

// Joints and matrices defining a skin.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Skin {
    /// [SkinId][tonner::skin::SkinId], if the resource has been loaded. Cleared once the scene has been loaded.
    #[serde(skip)]
    id: Option<tonner::geometry::skin::SkinId>,

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

impl Skin {
    pub(super) fn load(
        &mut self,
        nodes: &[Node],
        accessors: &[Accessor],
        buffer_views: &[BufferView],
        buffers: &[Buffer],
        skin_manager: &mut SkinManager,
    ) -> Result<SkinId> {
        if let Some(id) = self.id {
            return Ok(id);
        }

        let mut joints = Vec::with_capacity(self.joints.len());
        for (joint_idx, &index) in self.joints.iter().enumerate() {
            joints.push(
                nodes
                    .get(index)
                    .with_context(|| format!("skin.nodes[{joint_idx}] {index} is out of range."))?
                    .id
                    .with_context(|| {
                        format!("skin.nodes[{joint_idx}] {index} is not part of scene.")
                    })?,
            );
        }

        let mut builder = SkinBuilder::default().nodes(joints);
        if let Some(inverse_bind_matrices) = self.inverse_bind_matrices {
            struct RegisterBindMatrices {
                builder: SkinBuilder,
            }

            impl IteratorConsumer<'_, Mat4> for RegisterBindMatrices {
                type Return = SkinBuilder;

                fn consume<I: Iterator<Item = Mat4>>(self, iter: I) -> Result<Self::Return> {
                    Ok(self.builder.inverse_bind_matrices(iter))
                }
            }

            let consumer = RegisterBindMatrices { builder };
            let accessor = accessors.get(inverse_bind_matrices).with_context(|| {
                format!("inverse_bind_matrices {inverse_bind_matrices} is out of range")
            })?;

            builder = accessor.iter_mat4(buffer_views, buffers, consumer)?;
        }

        let skin = builder.build().unwrap();
        let id = skin.id();
        skin_manager.skins.insert(skin);
        Ok(id)
    }
}
