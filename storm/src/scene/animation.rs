use std::time::Duration;

use approx::abs_diff_eq;
use glam::{Quat, Vec3, Vec4};

use crate::{DenseEntry, Id, storage::SparseSet};

use super::{Node, Scene};

pub struct Animation {
    id: Id<Self>,
    name: String,
    channels: Vec<Channel>,
    duration: f32,
    current_timestamp: f32,
    repeat: bool,
}

impl DenseEntry for Animation {
    type Key = Self;

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}

impl Animation {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn current_timestamp(&self) -> f32 {
        self.current_timestamp
    }

    pub fn repeat(&self) -> bool {
        self.repeat
    }

    fn update(&self, nodes: &mut SparseSet<Node>) {
        self.channels.iter().for_each(|channel| {
            let node = &mut nodes[channel.node];
            match &channel.outputs {
                Outputs::Translations(slice) => {
                    node.local_transform.set_translation(interpolate_vec3(
                        self.current_timestamp,
                        &channel.inputs,
                        channel.interpolation,
                        &slice,
                    ))
                }
                Outputs::Rotations(slice) => node.local_transform.set_rotation(interpolate_quat(
                    self.current_timestamp,
                    &channel.inputs,
                    channel.interpolation,
                    &slice,
                )),
                Outputs::Scales(slice) => node.local_transform.set_scale(interpolate_vec3(
                    self.current_timestamp,
                    &channel.inputs,
                    channel.interpolation,
                    &slice,
                )),
                Outputs::Weights(slice, count) => {
                    node.weights = interpolate_weights(
                        self.current_timestamp,
                        &channel.inputs,
                        channel.interpolation,
                        &slice,
                        *count,
                    )
                }
            }
        })
    }
}

#[must_use]
pub struct AnimationBuilder<'s> {
    scene: &'s mut Scene,
    name: Option<String>,
    channels: Vec<Channel>,
    duration: f32,
    repeat: bool,
}

impl<'s> AnimationBuilder<'s> {
    pub fn new(scene: &'s mut Scene) -> Self {
        Self {
            scene,
            name: None,
            channels: Vec::new(),
            duration: 0.0,
            repeat: false,
        }
    }

    pub fn name(mut self, name: impl Into<Option<String>>) -> Self {
        self.name = name.into();
        self
    }

    pub fn repeat(mut self) -> Self {
        self.repeat = true;
        self
    }

    pub fn channels(mut self, channels: impl IntoIterator<Item = Channel>) -> Self {
        self.channels.extend(channels.into_iter().map(|channel| {
            let duration = *channel.inputs.last().unwrap();
            if duration > self.duration {
                self.duration = duration;
            }
            channel
        }));
        self
    }

    pub fn build(self) -> &'s mut Animation {
        let id = self.scene.animations.next_id();
        self.scene.animations.insert(Animation {
            id,
            name: self.name.unwrap_or_else(|| format!("Animation {id}")),
            channels: self.channels,
            duration: self.duration,
            current_timestamp: 0.0,
            repeat: self.repeat,
        })
    }
}

impl Scene {
    pub fn animation_builder(&mut self) -> AnimationBuilder {
        AnimationBuilder::new(self)
    }

    pub fn animations(&self) -> std::slice::Iter<'_, Animation> {
        self.animations.iter()
    }

    pub fn play_animation(&mut self, animation: Id<Animation>) {
        self.animations[animation].current_timestamp = 0.0;
        self.playing_animations.insert(animation);
    }

    pub fn resume_animation(&mut self, animation: Id<Animation>) {
        self.playing_animations.insert(animation);
    }

    pub fn animation_is_playing(&self, animation: Id<Animation>) -> bool {
        self.playing_animations.contains(animation)
    }

    pub fn pause_animation(&mut self, animation: Id<Animation>) {
        self.playing_animations.remove(animation);
    }

    pub fn stop_animation(&mut self, animation: Id<Animation>) {
        let animation = &mut self.animations[animation];
        animation.current_timestamp = 0.0;
        animation.update(&mut self.nodes);
        let nodes: Vec<Id<Node>> = animation
            .channels
            .iter()
            .map(|channel| channel.node)
            .collect();
        let animation = animation.id();
        for node in nodes {
            self.node_handle(node).update_world_matrices();
        }
        self.playing_animations.remove(animation);
    }

    pub fn repeat_animation(&mut self, animation: Id<Animation>, repeat: bool) {
        self.animations[animation].repeat = repeat;
    }

    pub(super) fn update_animations(&mut self, delta_time: Duration) {
        let delta_time = delta_time.as_secs_f32();
        let ended: Vec<Id<Animation>> = self
            .playing_animations
            .iter_mut()
            .filter_map(|animation| {
                let animation = &mut self.animations[*animation];
                animation.current_timestamp += delta_time;
                let ended = if animation.current_timestamp > animation.duration {
                    if animation.repeat {
                        animation.current_timestamp -= animation.duration;
                        None
                    } else {
                        animation.current_timestamp = 0.0;
                        Some(animation.id)
                    }
                } else {
                    None
                };
                animation.update(&mut self.nodes);
                ended
            })
            .collect();
        for id in ended {
            self.playing_animations.remove(id);
        }
    }
}

pub struct Channel {
    pub node: Id<Node>,
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
