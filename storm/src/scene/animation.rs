use std::{collections::HashMap, time::Duration};

use approx::abs_diff_eq;
use glam::{Quat, Vec3, Vec4};
use thiserror::Error;
use uuid::Uuid;

use crate::scene::{NodeManager, node::NodeId};

use super::Scene;

/// A unique id for an animation. An animation have one and only one id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnimationId(Uuid);

struct AnimationData {
    id: AnimationId,
    name: String,
    channels: Vec<Channel>,
    duration: f32,
    current_timestamp: f32,
    repeat: bool,
    playing: bool,
}

impl AnimationData {
    fn update(&self, node_manager: &mut NodeManager) -> Result<(), SimulateAnimationError> {
        for channel in &self.channels {
            match &channel.outputs {
                Outputs::Translations(slice) => {
                    node_manager
                        .set_local_translation(
                            channel.node,
                            interpolate_vec3(
                                self.current_timestamp,
                                &channel.inputs,
                                channel.interpolation,
                                &slice,
                            ),
                        )
                        .map_err(|()| SimulateAnimationError::InvalidNode(channel.node))?;
                }
                Outputs::Rotations(slice) => {
                    node_manager
                        .set_local_rotation(
                            channel.node,
                            interpolate_quat(
                                self.current_timestamp,
                                &channel.inputs,
                                channel.interpolation,
                                &slice,
                            ),
                        )
                        .map_err(|()| SimulateAnimationError::InvalidNode(channel.node))?;
                }
                Outputs::Scales(slice) => {
                    node_manager
                        .set_local_scale(
                            channel.node,
                            interpolate_vec3(
                                self.current_timestamp,
                                &channel.inputs,
                                channel.interpolation,
                                &slice,
                            ),
                        )
                        .map_err(|()| SimulateAnimationError::InvalidNode(channel.node))?;
                }
                Outputs::Weights(_slice, _count) => {
                    // node.weights = interpolate_weights(
                    //     self.current_timestamp,
                    //     &channel.inputs,
                    //     channel.interpolation,
                    //     &slice,
                    //     *count,
                    // )
                    todo!()
                }
            }
        }
        Ok(())
    }
}

/// A builder for animations.
#[must_use]
pub struct AnimationBuilder {
    name: Option<String>,
    channels: Vec<Channel>,
    duration: f32,
    repeat: bool,
}

impl AnimationBuilder {
    pub fn new() -> Self {
        Self {
            name: None,
            channels: Vec::new(),
            duration: 0.0,
            repeat: false,
        }
    }

    pub fn name(self, name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            ..self
        }
    }

    pub fn repeat(self) -> Self {
        Self {
            repeat: true,
            ..self
        }
    }

    pub fn build(self, scene: &mut Scene) -> Result<AnimationId, AnimationBuilderError> {
        for channel in &self.channels {
            if !scene.node_manager.contains(channel.node) {
                return Err(AnimationBuilderError::InvalidNode(channel.node));
            }
        }

        let id = AnimationId(Uuid::new_v4());
        let data = AnimationData {
            id,
            name: self.name.unwrap_or_default(),
            channels: self.channels,
            duration: self.duration,
            current_timestamp: 0.0,
            repeat: self.repeat,
            playing: false,
        };
        scene.animation_manager.animations.insert(id, data);
        Ok(id)
    }
}

#[derive(Debug, Error)]
pub enum AnimationBuilderError {
    #[error("invalid node {0}")]
    InvalidNode(NodeId),
}

/// Manages all animations, their shared data as well as their update logic.
pub(super) struct AnimationManager {
    animations: HashMap<AnimationId, AnimationData>,
}

impl AnimationManager {
    pub(super) fn new() -> Self {
        Self {
            animations: HashMap::new(),
        }
    }

    /// Start the animation. This will reset the animation if paused or if it is already playing.
    /// This function will fails if the animation does not exist.
    pub(super) fn start(&mut self, animation: AnimationId) -> Result<(), ()> {
        let data = self.animations.get_mut(&animation).ok_or(())?;
        data.current_timestamp = 0.0;
        data.playing = true;
        Ok(())
    }

    /// Resume the animation. If the animation never run, this is the same as [`AnimationManager::start`].
    /// This function will fails if the animation does not exist.
    pub(super) fn resume(&mut self, animation: AnimationId) -> Result<(), ()> {
        self.animations.get_mut(&animation).ok_or(())?.playing = true;
        Ok(())
    }

    /// Pause the animation and leave the nodes in their current states.
    /// This function will fails if the animation does not exist.
    pub(super) fn pause(&mut self, animation: AnimationId) -> Result<(), ()> {
        self.animations.get_mut(&animation).ok_or(())?.playing = false;
        Ok(())
    }

    /// Stops the animation and reset the nodes to their initial states.
    /// This function will fails if the animation does not exist.
    pub(super) fn stop(
        &mut self,
        animation: AnimationId,
        node_manager: &mut NodeManager,
    ) -> Result<(), ()> {
        let data = self.animations.get_mut(&animation).ok_or(())?;
        data.playing = false;
        data.current_timestamp = 0.0;
        data.update(node_manager);
        Ok(())
    }

    pub(super) fn simulate(
        &mut self,
        duration: Duration,
        node_manager: &mut NodeManager,
    ) -> Result<(), SimulateAnimationError> {
        let delta_time = duration.as_secs_f32();
        for animation in self.animations.values_mut().filter(|data| data.playing) {
            animation.current_timestamp += delta_time;
            if animation.current_timestamp > animation.duration {
                if animation.repeat {
                    animation.current_timestamp -= animation.duration
                } else {
                    animation.current_timestamp = 0.0;
                    animation.playing = false;
                }
            }
            animation.update(node_manager)?;
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub(super) enum SimulateAnimationError {
    #[error("invalid node: {0}")]
    InvalidNode(NodeId),
}

struct Channel {
    pub node: NodeId,
    pub inputs: Vec<f32>,
    pub interpolation: Interpolation,
    pub outputs: Outputs,
}

#[derive(Debug, Clone, Copy)]
pub enum Interpolation {
    Step,
    Linear,
    CubicSpline,
}

#[derive(Debug)]
pub enum Outputs {
    Translations(Vec<[f32; 3]>),
    Rotations(Vec<[f32; 4]>),
    Scales(Vec<[f32; 3]>),
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
