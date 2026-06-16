use std::{collections::HashMap, fmt::Display, time::Duration};

use anyhow::{Context, Result};
use glam::{Vec3, Vec4};
use serde::{Deserialize, Serialize};
use tempete::mesh::{MeshInstance, MeshInstanceId};
use storm_animation::key_frame::{
    Interpolation, KeyFrameChannel, MeshInstanceChannel, NodeChannel, NodeOutputs,
};

use crate::{Accessor, Buffer, BufferView, Node, accessor::IteratorConsumer};

/// A keyframe animation.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Animation {
    /// An array of animation channels. An animation channel combines an
    /// animation sampler with a target property being animated. Different
    /// channels of the same animation **MUST NOT** have the same targets.
    channels: Vec<AnimationChannel>,

    /// An array of animation samplers. An animation sampler combines timestamps
    /// with a sequence of output values and defines an interpolation algorithm.
    samplers: Vec<AnimationSampler>,

    /// The user-defined name of this object. This is not necessarily unique, e.g.,
    /// an accessor and a buffer could have the same name, or two accessors could
    /// even have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

impl Animation {
    pub(super) fn load(
        &self,
        nodes: &[Node],
        accessors: &[Accessor],
        buffer_views: &[BufferView],
        buffers: &[Buffer],
        mesh_instaces: &HashMap<MeshInstanceId, MeshInstance>,
    ) -> Result<storm_animation::Animation> {
        let mut duration = 0.0;
        let mut channels: Vec<Box<dyn storm_animation::AnimationChannel>> =
            Vec::with_capacity(self.channels.len());

        for (channel_idx, channel) in self.channels.iter().enumerate() {
            let channel_ctx = move || format!("Failed to load animation.channel {channel_idx}.");

            let node_idx = match channel.target.node {
                Some(idx) => idx,
                None => continue,
            };
            let node = match nodes
                .get(node_idx)
                .with_context(|| format!("channel.target.node {node_idx} is out of range."))
                .with_context(channel_ctx)?
                .id
            {
                Some(id) => id,
                None => continue,
            };

            let sampler = self
                .samplers
                .get(channel.sampler)
                .with_context(|| format!("channel.sampler {} is out of range.", channel.sampler))
                .with_context(channel_ctx)?;

            let sampler_ctx = || format!("Failed to load channel.sampler {}.", channel.sampler);

            let inputs = {
                let accessor = accessors
                    .get(sampler.input)
                    .with_context(|| format!("sampler.input {} is out of range.", sampler.input))
                    .with_context(sampler_ctx)
                    .with_context(channel_ctx)?;

                struct InputsConsumer;

                impl<'a> IteratorConsumer<'a, f32> for InputsConsumer {
                    type Return = Vec<f32>;

                    fn consume<I: Iterator<Item = f32> + 'a>(
                        self,
                        iter: I,
                    ) -> Result<Self::Return> {
                        Ok(iter.collect())
                    }
                }

                accessor
                    .iter_f32(buffer_views, buffers, InputsConsumer)
                    .with_context(|| format!("Failed to load sampler.input {}.", sampler.input))
                    .with_context(sampler_ctx)
                    .with_context(channel_ctx)?
            };

            if let Some(&channel_duration) = inputs.last() {
                if channel_duration > duration {
                    duration = channel_duration;
                }
            }

            let interpolation = match sampler.interpolation {
                AnimationInterpolation::Step => Interpolation::Step,
                AnimationInterpolation::Linear => Interpolation::Linear,
                AnimationInterpolation::Cubicspline => Interpolation::CubicSpline,
            };

            let channel = {
                let accessor = accessors
                    .get(sampler.output)
                    .with_context(|| format!("sampler.output {} is out of range.", sampler.output))
                    .with_context(sampler_ctx)
                    .with_context(channel_ctx)?;

                let output_ctx = || format!("Failed to load sampler.output {}.", sampler.output);

                struct OutputsConsumer;

                impl<'a> IteratorConsumer<'a, Vec3> for OutputsConsumer {
                    type Return = Vec<[f32; 3]>;

                    fn consume<I: Iterator<Item = Vec3> + 'a>(
                        self,
                        iter: I,
                    ) -> Result<Self::Return> {
                        Ok(iter.map(|v| v.to_array()).collect())
                    }
                }

                impl<'a> IteratorConsumer<'a, Vec4> for OutputsConsumer {
                    type Return = Vec<[f32; 4]>;

                    fn consume<I: Iterator<Item = Vec4> + 'a>(
                        self,
                        iter: I,
                    ) -> Result<Self::Return> {
                        Ok(iter.map(|v| v.to_array()).collect())
                    }
                }

                impl<'a> IteratorConsumer<'a, f32> for OutputsConsumer {
                    type Return = Vec<f32>;

                    fn consume<I: Iterator<Item = f32> + 'a>(
                        self,
                        iter: I,
                    ) -> Result<Self::Return> {
                        Ok(iter.collect())
                    }
                }

                match channel.target.path {
                    AnimationTargetPath::Translation => {
                        let outputs = NodeOutputs::Translations(
                            accessor
                                .iter_vec3(buffer_views, buffers, OutputsConsumer)
                                .with_context(output_ctx)
                                .with_context(sampler_ctx)
                                .with_context(channel_ctx)?,
                        );
                        KeyFrameChannel::Node(NodeChannel {
                            node,
                            inputs,
                            interpolation,
                            outputs,
                        })
                    }
                    AnimationTargetPath::Rotation => {
                        let outputs = NodeOutputs::Rotations(
                            accessor
                                .iter_vec4(buffer_views, buffers, OutputsConsumer)
                                .with_context(output_ctx)
                                .with_context(sampler_ctx)
                                .with_context(channel_ctx)?,
                        );
                        KeyFrameChannel::Node(NodeChannel {
                            node,
                            inputs,
                            interpolation,
                            outputs,
                        })
                    }
                    AnimationTargetPath::Scale => {
                        let outputs = NodeOutputs::Scales(
                            accessor
                                .iter_vec3(buffer_views, buffers, OutputsConsumer)
                                .with_context(output_ctx)
                                .with_context(sampler_ctx)
                                .with_context(channel_ctx)?,
                        );
                        KeyFrameChannel::Node(NodeChannel {
                            node,
                            inputs,
                            interpolation,
                            outputs,
                        })
                    }
                    AnimationTargetPath::Weights => {
                        let (id, instance) = match mesh_instaces
                            .iter()
                            .find(|(_, instance)| instance.entity == node)
                        {
                            Some((&id, instance)) => (id, instance),
                            None => continue,
                        };

                        let weights = accessor
                            .iter_f32(buffer_views, buffers, OutputsConsumer)
                            .with_context(output_ctx)
                            .with_context(sampler_ctx)
                            .with_context(channel_ctx)?;
                        KeyFrameChannel::MeshInstance(MeshInstanceChannel {
                            instance: id,
                            inputs,
                            interpolation,
                            weights,
                            morph_target_count: instance.mesh().morph_target_count(),
                        })
                    }
                }
            };

            channels.push(Box::new(channel));
        }

        Ok(storm_animation::Animation {
            name: self.name.clone().unwrap_or_default(),
            channels,
            repeat: true,
            progress: Duration::ZERO,
            duration: Duration::from_secs_f32(duration),
        })
    }
}

/// An animation channel combines an animation sampler with a target property being animated.
#[derive(Debug, Serialize, Deserialize)]
struct AnimationChannel {
    /// The index of a sampler in this animation used to compute the value for the target,
    /// e.g., a node’s translation, rotation, or scale (TRS).
    sampler: usize,

    /// The descriptor of the animated property.
    target: AnimationTarget,
}

/// The descriptor of the animated property.
#[derive(Debug, Serialize, Deserialize)]
struct AnimationTarget {
    /// The index of the node to animate. When undefined, the animated object
    /// **MAY** be defined by an extension.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    node: Option<usize>,

    /// The name of the node’s TRS property to animate, or the "weights" of
    /// the Morph Targets it instantiates. For the [Translation](AnimationTargetPath::Translation)
    /// property, the values that are provided by the sampler are the translation along
    /// the X, Y, and Z axes. For the [Rotation](AnimationTargetPath::Rotation) property,
    /// the values are a quaternion in the order (x, y, z, w), where w is the scalar.
    /// For the [Scale](AnimationTargetPath::Scale) property, the values are the scaling
    /// factors along the X, Y, and Z axes.
    path: AnimationTargetPath,
}

/// The name of the node’s TRS property to animate, or the "weights" of
/// the Morph Targets it instantiates. For the [Translation](AnimationTargetPath::Translation)
/// property, the values that are provided by the sampler are the translation along
/// the X, Y, and Z axes. For the [Rotation](AnimationTargetPath::Rotation) property,
/// the values are a quaternion in the order (x, y, z, w), where w is the scalar.
/// For the [Scale](AnimationTargetPath::Scale) property, the values are the scaling
/// factors along the X, Y, and Z axes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AnimationTargetPath {
    #[serde(rename = "translation")]
    Translation,

    #[serde(rename = "rotation")]
    Rotation,

    #[serde(rename = "scale")]
    Scale,

    #[serde(rename = "weights")]
    Weights,
}

impl Display for AnimationTargetPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Translation => "translation",
            Self::Rotation => "rotation",
            Self::Scale => "scale",
            Self::Weights => "weights",
        })
    }
}

/// An animation sampler combines timestamps with a sequence of output values and defines an interpolation algorithm.
#[derive(Debug, Serialize, Deserialize)]
struct AnimationSampler {
    /// The index of an accessor containing keyframe timestamps. The accessor **MUST** be of scalar type with
    /// floating-point components. The values represent time in seconds with `time[0] >= 0.0`, and strictly
    /// increasing values, i.e., `time[n + 1] > time[n]`.
    input: usize,

    /// Interpolation algorithm.
    #[serde(default)]
    #[serde(skip_serializing_if = "AnimationInterpolation::is_default")]
    interpolation: AnimationInterpolation,

    /// The index of an accessor, containing keyframe output values.
    output: usize,
}

/// Interpolation algorithm.
#[derive(Debug, Default, Serialize, Deserialize)]
enum AnimationInterpolation {
    /// The animated values are linearly interpolated between keyframes.
    /// When targeting a rotation, spherical linear interpolation (slerp)
    /// **SHOULD** be used to interpolate quaternions. The number of
    /// output elements **MUST** equal the number of input elements.
    #[default]
    #[serde(rename = "LINEAR")]
    Linear,

    /// The animated values remain constant to the output of the first keyframe,
    /// until the next keyframe. The number of output elements **MUST** equal the
    /// number of input elements.
    #[serde(rename = "STEP")]
    Step,

    /// The animation’s interpolation is computed using a cubic spline with
    /// specified tangents. The number of output elements **MUST** equal three
    /// times the number of input elements. For each input element, the output
    /// stores three elements, an in-tangent, a spline vertex, and an out-tangent.
    /// There **MUST** be at least two keyframes when using this interpolation.
    #[serde(rename = "CUBICSPLINE")]
    Cubicspline,
}

impl AnimationInterpolation {
    fn is_default(&self) -> bool {
        match self {
            AnimationInterpolation::Linear => true,
            _ => false,
        }
    }
}
