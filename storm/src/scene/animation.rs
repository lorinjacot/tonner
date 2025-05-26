use std::time::Duration;

use glam::{Quat, Vec3, Vec4};

use crate::{DenseEntry, Id, storage::SparseSet};

use super::{Node, Scene};

pub struct Animation {
    id: Id<Self>,
    name: String,
    duration: f32,
    channels: Vec<Channel>,
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

    fn update(&self, current_timestamp: f32, nodes: &mut SparseSet<Node>) {
        self.channels.iter().for_each(|channel| {
            let node = &mut nodes[channel.node];
            match &channel.outputs {
                Outputs::Translations(slice) => {
                    node.local_transform.set_translation(interpolate_vec3(
                        current_timestamp,
                        &channel.inputs,
                        channel.interpolation,
                        &slice,
                    ))
                }
                Outputs::Rotations(slice) => node.local_transform.set_rotation(interpolate_quat(
                    current_timestamp,
                    &channel.inputs,
                    channel.interpolation,
                    &slice,
                )),
                Outputs::Scales(slice) => node.local_transform.set_scale(interpolate_vec3(
                    current_timestamp,
                    &channel.inputs,
                    channel.interpolation,
                    &slice,
                )),
            }
        })
    }
}

#[must_use]
pub struct AnimationBuilder<'s> {
    scene: &'s mut Scene,
    name: Option<String>,
    duration: f32,
    channels: Vec<Channel>,
}

impl<'s> AnimationBuilder<'s> {
    pub fn new(scene: &'s mut Scene) -> Self {
        Self {
            scene,
            name: None,
            duration: 0.0,
            channels: Vec::new(),
        }
    }

    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
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
            duration: self.duration,
            channels: self.channels,
        })
    }
}

pub(super) struct PlayingAnimation {
    animation: Id<Animation>,
    current_timestamp: f32,
    should_loop: bool,
}

impl DenseEntry for PlayingAnimation {
    type Key = Animation;

    fn id(&self) -> Id<Self::Key> {
        self.animation
    }
}

impl Scene {
    pub fn animation_builder(&mut self) -> AnimationBuilder {
        AnimationBuilder::new(self)
    }

    pub fn animations(&self) -> std::slice::Iter<'_, Animation> {
        self.animations.iter()
    }

    pub fn play_animation(&mut self, animation: Id<Animation>, should_loop: bool) {
        self.playing_animations.insert(PlayingAnimation {
            animation,
            current_timestamp: 0.0,
            should_loop,
        });
    }

    pub fn animation_current_timestamp(&self, animation: Id<Animation>) -> Option<f32> {
        self.playing_animations
            .get(animation)
            .map(|animation| animation.current_timestamp)
    }

    pub(super) fn update_animations(&mut self, delta_time: Duration) {
        let delta_time = delta_time.as_secs_f32();
        let ended: Vec<Id<Animation>> = self
            .playing_animations
            .iter_mut()
            .filter_map(
                |PlayingAnimation {
                     animation,
                     current_timestamp,
                     should_loop,
                 }| {
                    *current_timestamp += delta_time;
                    let animation = &self.animations[*animation];
                    let ended = if *current_timestamp > animation.duration {
                        if *should_loop {
                            *current_timestamp -= animation.duration;
                            None
                        } else {
                            Some(animation.id)
                        }
                    } else {
                        None
                    };
                    animation.update(*current_timestamp, &mut self.nodes);
                    ended
                },
            )
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

pub enum Outputs {
    Translations(Vec<[f32; 3]>),
    Rotations(Vec<[f32; 4]>),
    Scales(Vec<[f32; 3]>),
}

fn interpolate_vec3(
    current_timestamp: f32,
    inputs: &[f32],
    interpolation: Interpolation,
    ouputs: &[[f32; 3]],
) -> Vec3 {
    interpolate(
        current_timestamp,
        inputs,
        interpolation,
        ouputs,
        |v_previous| Vec3::from_slice(v_previous),
        |t, v_previous, v_next| {
            let v_previous = Vec3::from_slice(v_previous);
            let v_next = Vec3::from_slice(v_next);
            (1.0 - t) * v_previous + t * v_next
        },
        |t, t_d, a_next, v_prev, v_next, b_prev| {
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
    ouputs: &[[f32; 4]],
) -> Quat {
    interpolate(
        current_timestamp,
        inputs,
        interpolation,
        ouputs,
        |v_previous| Quat::from_slice(v_previous),
        |t, v_previous, v_next| {
            let v_previous = Vec4::from_slice(v_previous);
            let v_next = Vec4::from_slice(v_next);
            let dot = v_previous.dot(v_next);
            let abs = dot.abs();
            let a = abs.acos();
            let s = dot / abs;
            let a_sin = a.sin();
            let v_t =
                (a * (1.0 - t)).sin() / a_sin * v_previous + s * (a * t).sin() / a_sin * v_next;
            Quat::from_vec4(v_t)
        },
        |t, t_d, a_next, v_prev, v_next, b_prev| {
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
    )
}

fn interpolate<const N: usize, T>(
    current_timestamp: f32,
    inputs: &[f32],
    interpolation: Interpolation,
    ouputs: &[[f32; N]],
    step_callback: impl FnOnce(&[f32; N]) -> T,
    linear_callback: impl FnOnce(f32, &[f32; N], &[f32; N]) -> T,
    cubic_spline_callback: impl FnOnce(f32, f32, &[f32; N], &[f32; N], &[f32; N], &[f32; N]) -> T,
) -> T {
    let t_c = current_timestamp;
    let mut iter = inputs.iter().enumerate();
    while let Some((k, t_k)) = iter.next() {
        if t_c == *t_k {
            return step_callback(&ouputs[k]);
        } else if t_c < *t_k {
            if k == 0 {
                return step_callback(ouputs.first().unwrap());
            } else {
                return match interpolation {
                    Interpolation::Step => step_callback(&ouputs[k - 1]),
                    Interpolation::Linear => {
                        let t_previous = inputs[k - 1];
                        let v_previous = &ouputs[k - 1];
                        let t_next = *t_k;
                        let v_next = &ouputs[k];
                        let t_d = t_next - t_previous;
                        let t = (t_c - t_k) / t_d;
                        linear_callback(t, v_previous, v_next)
                    }
                    Interpolation::CubicSpline => {
                        let t_previous = inputs[k - 1];
                        let t_next = *t_k;
                        let t_d = t_next - t_previous;
                        let t = (t_c - t_k) / t_d;

                        let n = inputs.len();
                        let k_prev = k - 1;
                        let k_next = k;
                        let b_prev = &ouputs[n + n + k_prev];
                        let v_prev = &ouputs[n + k_prev];
                        let v_next = &ouputs[n + k_next];
                        let a_next = &ouputs[k_next];

                        cubic_spline_callback(t, t_d, a_next, v_prev, v_next, b_prev)
                    }
                };
            }
        }
    }
    step_callback(ouputs.last().unwrap())
}
