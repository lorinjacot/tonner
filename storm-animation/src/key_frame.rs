use approx::abs_diff_eq;
use glam::{Quat, Vec3, Vec4};
use storm::{mesh::MeshInstanceId, scene_graph::NodeId};

use crate::{AnimationChannel, AnimationError};

/// Node animation base on `input`/`output` pairs.
/// The `output` contains the value the node should take
/// at time `input`.
///
/// See https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#animations
/// and https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#appendix-c-interpolation
/// for more informations.
#[derive(Debug)]
pub enum KeyFrameChannel {
    Node(NodeChannel),
    MeshInstance(MeshInstanceChannel),
}

#[derive(Debug)]
pub struct NodeChannel {
    /// The node modified by this channel.
    pub node: NodeId,
    /// Linear time in seconds.
    pub inputs: Vec<f32>,
    /// Sets the interpolation method between two `input`/`output`
    /// pairs.
    pub interpolation: Interpolation,
    /// Animated property values.
    pub outputs: NodeOutputs,
}

#[derive(Debug)]
pub struct MeshInstanceChannel {
    /// The mesh instance modified by this channel.
    pub instance: MeshInstanceId,
    /// Linear time in seconds.
    pub inputs: Vec<f32>,
    /// Sets the interpolation method between two `input`/`output`
    /// pairs.
    pub interpolation: Interpolation,
    /// Animated property values.
    pub weights: Vec<f32>,
    pub morph_target_count: usize,
}

impl AnimationChannel for KeyFrameChannel {
    fn update(
        &mut self,
        progress: std::time::Duration,
        _duration: std::time::Duration,
        animatable: &mut crate::Animatable,
    ) -> Result<(), AnimationError> {
        let progress = progress.as_secs_f32();
        match self {
            KeyFrameChannel::Node(NodeChannel {
                node,
                inputs,
                interpolation,
                outputs,
            }) => match outputs {
                NodeOutputs::Translations(slice) => {
                    animatable.scene_graph.set_local_transformation(
                        *node,
                        interpolate_vec3(progress, inputs, *interpolation, slice),
                        None,
                        None,
                    )?;
                }
                NodeOutputs::Rotations(slice) => {
                    animatable.scene_graph.set_local_transformation(
                        *node,
                        None,
                        interpolate_quat(progress, inputs, *interpolation, slice),
                        None,
                    )?;
                }
                NodeOutputs::Scales(slice) => {
                    animatable.scene_graph.set_local_transformation(
                        *node,
                        None,
                        None,
                        interpolate_vec3(progress, inputs, *interpolation, slice),
                    )?;
                }
            },
            KeyFrameChannel::MeshInstance(MeshInstanceChannel {
                instance,
                inputs,
                interpolation,
                weights,
                morph_target_count,
            }) => {
                animatable
                    .mesh_instance
                    .get_mut(instance)
                    .unwrap()
                    .set_weights(&interpolate_weights(
                        progress,
                        inputs,
                        *interpolation,
                        weights,
                        *morph_target_count,
                    ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interpolation {
    Step,
    Linear,
    CubicSpline,
}

#[derive(Debug)]
pub enum NodeOutputs {
    Translations(Vec<[f32; 3]>),
    Rotations(Vec<[f32; 4]>),
    Scales(Vec<[f32; 3]>),
}

#[derive(Debug)]
pub enum MeshInstanceOutputs {
    Weights(Vec<f32>, usize),
}

fn interpolate_vec3(
    current_timestamp: f32,
    inputs: &[f32],
    interpolation: Interpolation,
    outputs: &[[f32; 3]],
) -> Vec3 {
    interpolate(
        current_timestamp,
        inputs,
        interpolation,
        outputs,
        |v_previous| Vec3::from_slice(v_previous),
        |t, v_previous, v_next| {
            let v_previous = Vec3::from_slice(v_previous);
            let v_next = Vec3::from_slice(v_next);
            (1.0 - t) * v_previous + t * v_next
        },
        |t, t_d, v_prev, b_prev, a_next, v_next| {
            let a_next = Vec3::from_slice(a_next);
            let v_prev = Vec3::from_slice(v_prev);
            let v_next = Vec3::from_slice(v_next);
            let b_prev = Vec3::from_slice(b_prev);

            let t2 = t * t;
            let t3 = t2 * t;

            (2.0 * t3 - 3.0 * t2 + 1.0) * v_prev
                + t_d * (t3 - 2.0 * t2 + t) * b_prev
                + (-2.0 * t3 + 3.0 * t2) * v_next
                + t_d * (t3 - t2) * a_next
        },
    )
}

fn interpolate_quat(
    current_timestamp: f32,
    inputs: &[f32],
    interpolation: Interpolation,
    outputs: &[[f32; 4]],
) -> Quat {
    let v = interpolate(
        current_timestamp,
        inputs,
        interpolation,
        outputs,
        |v_previous| Quat::from_slice(v_previous),
        |t, v_previous, v_next| {
            let v_previous = Vec4::from_slice(v_previous);
            let v_next = Vec4::from_slice(v_next);
            let dot = v_previous.dot(v_next);
            let abs = dot.abs();
            let a = abs.acos();
            let v_t = if a.is_nan() || abs_diff_eq!(a, 0.0) {
                (1.0 - t) * v_previous + t * v_next
            } else {
                let s = dot / abs;
                let a_sin = a.sin();
                (a * (1.0 - t)).sin() / a_sin * v_previous + s * (a * t).sin() / a_sin * v_next
            };
            Quat::from_vec4(v_t).normalize()
        },
        |t, t_d, v_prev, b_prev, a_next, v_next| {
            let a_next = Vec4::from_slice(a_next);
            let v_prev = Vec4::from_slice(v_prev);
            let v_next = Vec4::from_slice(v_next);
            let b_prev = Vec4::from_slice(b_prev);

            let t2 = t * t;
            let t3 = t2 * t;

            let v_t = (2.0 * t3 - 3.0 * t2 + 1.0) * v_prev
                + t_d * (t3 - 2.0 * t2 + t) * b_prev
                + (-2.0 * t3 + 3.0 * t2) * v_next
                + t_d * (t3 - t2) * a_next;
            Quat::from_vec4(v_t).normalize()
        },
    );
    v
}

fn interpolate<const N: usize, T>(
    current_timestamp: f32,
    inputs: &[f32],
    interpolation: Interpolation,
    outputs: &[[f32; N]],
    step_callback: impl FnOnce(&[f32; N]) -> T,
    linear_callback: impl FnOnce(f32, &[f32; N], &[f32; N]) -> T,
    cubic_spline_callback: impl FnOnce(f32, f32, &[f32; N], &[f32; N], &[f32; N], &[f32; N]) -> T,
) -> T {
    let t_c = current_timestamp;
    let mut iter = inputs.iter().enumerate();
    while let Some((k, t_k)) = iter.next() {
        if t_c == *t_k {
            return match interpolation {
                Interpolation::CubicSpline => step_callback(&outputs[3 * k + 1]),
                _ => step_callback(&outputs[k]),
            };
        } else if t_c < *t_k {
            if k == 0 {
                return match interpolation {
                    Interpolation::CubicSpline => step_callback(&outputs[1]),
                    _ => step_callback(&outputs[0]),
                };
            } else {
                return match interpolation {
                    Interpolation::Step => step_callback(&outputs[k - 1]),
                    Interpolation::Linear => {
                        let t_previous = inputs[k - 1];
                        let v_previous = &outputs[k - 1];
                        let t_next = *t_k;
                        let v_next = &outputs[k];
                        let t_d = t_next - t_previous;
                        let t = (t_c - t_previous) / t_d;
                        linear_callback(t, v_previous, v_next)
                    }
                    Interpolation::CubicSpline => {
                        let t_previous = inputs[k - 1];
                        let t_next = *t_k;
                        let t_d = t_next - t_previous;
                        let t = (t_c - t_previous) / t_d;

                        let start = (k - 1) * 3 + 1;
                        let v_prev = &outputs[start + 0];
                        let b_prev = &outputs[start + 1];
                        let a_next = &outputs[start + 2];
                        let v_next = &outputs[start + 3];

                        cubic_spline_callback(t, t_d, v_prev, b_prev, a_next, v_next)
                    }
                };
            }
        }
    }
    match interpolation {
        Interpolation::CubicSpline => step_callback(&outputs[3 * inputs.len() - 2]),
        _ => step_callback(outputs.last().unwrap()),
    }
}

fn interpolate_weights(
    current_timestamp: f32,
    inputs: &[f32],
    interpolation: Interpolation,
    outputs: &[f32],
    count: usize,
) -> Vec<f32> {
    let t_c = current_timestamp;
    let mut iter = inputs.iter().enumerate();
    while let Some((k, t_k)) = iter.next() {
        if t_c == *t_k {
            return match interpolation {
                Interpolation::CubicSpline => {
                    let start = k * 3 * count + count;
                    outputs[start..start + count].to_vec()
                }
                _ => {
                    let start = k * count;
                    outputs[start..start + count].to_vec()
                }
            };
        } else if t_c < *t_k {
            if k == 0 {
                return match interpolation {
                    Interpolation::CubicSpline => outputs[count..count + count].to_vec(),
                    _ => outputs[0..count].to_vec(),
                };
            } else {
                return match interpolation {
                    Interpolation::Step => {
                        let start = (k - 1) * count;
                        outputs[start..start + count].to_vec()
                    }
                    Interpolation::Linear => {
                        let t_previous = inputs[k - 1];
                        let mut start = (k - 1) * count;
                        let v_previous = &outputs[start..start + count];

                        let t_next = *t_k;
                        start += count;
                        let v_next = &outputs[start..start + count];

                        let t_d = t_next - t_previous;
                        let t = (t_c - t_previous) / t_d;

                        v_previous
                            .iter()
                            .zip(v_next)
                            .map(|(v_prev, v_next)| (1.0 - t) * v_prev + t * v_next)
                            .collect()
                    }
                    Interpolation::CubicSpline => {
                        let t_previous = inputs[k - 1];
                        let t_next = *t_k;
                        let t_d = t_next - t_previous;
                        let t = (t_c - t_previous) / t_d;

                        let mut start = (k - 1) * 3 * count + count;
                        let v_prev = &outputs[start..start + count];

                        start += count;
                        let b_prev = &outputs[start..start + count];

                        start += count;
                        let a_next = &outputs[start..start + count];

                        start += count;
                        let v_next = &outputs[start..start + count];

                        v_prev
                            .iter()
                            .zip(b_prev)
                            .zip(a_next)
                            .zip(v_next)
                            .map(|(((v_prev, b_prev), a_next), v_next)| {
                                let t2 = t * t;
                                let t3 = t2 * t;

                                (2.0 * t3 - 3.0 * t2 + 1.0) * v_prev
                                    + t_d * (t3 - 2.0 * t2 + t) * b_prev
                                    + (-2.0 * t3 + 3.0 * t2) * v_next
                                    + t_d * (t3 - t2) * a_next
                            })
                            .collect()
                    }
                };
            }
        }
    }
    match interpolation {
        Interpolation::CubicSpline => {
            outputs[inputs.len() * 3 * count - 2 * count..inputs.len() * 3 * count - count].to_vec()
        }
        _ => outputs[inputs.len() * count - count..inputs.len()].to_vec(),
    }
}
