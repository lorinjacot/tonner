use std::{collections::HashMap, process::Output, time::Duration};

use glam::{Mat4, Quat, Vec3, Vec4};

use crate::asset::Asset;

use super::{set_node_local_transform, Node};

pub struct AnimationManager {
    animations: Vec<Animation>,
}

impl AnimationManager {
    pub fn load(asset: &Asset, nodes_mapping: HashMap<usize, usize>) -> Self {
        let mut animations = Vec::with_capacity(asset.document.animations().len());
        asset.document.animations().for_each(|gltf_animation| {
            let mut duration = 0.0;
            let mut channels = Vec::new();
            for gltf_channel in gltf_animation.channels() {
                let accessor = gltf_channel.sampler().input();
                let view = accessor.view().expect("sparse accessor are not supported");
                let start = view.offset() + accessor.offset();
                let end = start + accessor.count() * accessor.size();
                let input = bytemuck::cast_slice(&asset.buffers[view.buffer().index()][start..end])
                    .to_vec();
                duration = (accessor.max().unwrap().as_array().unwrap()[0]
                    .as_f64()
                    .unwrap() as f32)
                    .max(duration);

                let node_index = gltf_channel.target().node().index();
                match nodes_mapping.get(&node_index) {
                    None => return,
                    Some(node_id) => match gltf_channel.target().property() {
                        gltf::animation::Property::MorphTargetWeights => {
                            panic!("morph target weights animations are not supported")
                        }
                        gltf::animation::Property::Rotation => {
                            let decomposed = asset
                                .document
                                .nodes()
                                .find(|node| node.index() == node_index)
                                .unwrap()
                                .transform()
                                .decomposed();
                            let original = TRS {
                                translation: Vec3::from_array(decomposed.0),
                                rotation: Quat::from_array(decomposed.1),
                                scale: Vec3::from_array(decomposed.2),
                            };
                            let interpolation = match gltf_channel.sampler().interpolation() {
                                gltf::animation::Interpolation::CubicSpline => {
                                    panic!("Cubic spline interpolations are not supported")
                                }
                                gltf::animation::Interpolation::Linear => Interpolation::Linear,
                                gltf::animation::Interpolation::Step => Interpolation::Step,
                            };
                            let accessor = gltf_channel.sampler().output();
                            let view = accessor.view().expect("sparse accessor not supported");
                            let start = view.offset() + view.offset();
                            let end = start + accessor.count() * accessor.size();
                            assert_eq!(
                                accessor.data_type(),
                                gltf::accessor::DataType::F32,
                                "only float currently supported"
                            );
                            let output = bytemuck::cast_slice(
                                &asset.buffers[view.buffer().index()][start..end],
                            )
                            .iter()
                            .map(|quat: &Quat| quat.normalize())
                            .collect::<Vec<_>>();
                            let path = ChannelPath::Rotation {
                                original,
                                interpolation,
                                output,
                            };

                            channels.push(Channel {
                                node: *node_id,
                                path,
                                input,
                            });
                        }
                        _ => todo!(),
                    },
                };
            }

            animations.push(Animation {
                channels,
                current_time: 0.0,
                duration,
            });
        });

        Self { animations }
    }

    pub fn update(&mut self, delta_time: Duration, nodes: &mut Vec<Node>) {
        for animation in &mut self.animations {
            animation.update(delta_time, nodes);
        }
    }
}

#[derive(Debug)]
struct Animation {
    channels: Vec<Channel>,
    current_time: f32,
    duration: f32,
}

impl Animation {
    fn update(&mut self, delta_time: Duration, nodes: &mut Vec<Node>) {
        self.current_time += delta_time.as_secs_f32();
        if self.current_time > self.duration {
            self.current_time = 0.0;
        }

        for channel in &self.channels {
            channel.update(self.current_time, nodes);
        }
    }
}

#[derive(Debug)]
struct Channel {
    node: usize,
    path: ChannelPath,
    input: Vec<f32>,
}

impl Channel {
    fn update(&self, current_time: f32, nodes: &mut Vec<Node>) {
        match &self.path {
            ChannelPath::Rotation {
                original,
                interpolation,
                output,
            } => {
                let mut transform = original.clone();

                transform.rotation = if current_time <= *self.input.first().unwrap() {
                    *output.first().unwrap() * original.rotation
                } else if current_time >= *self.input.last().unwrap() {
                    *output.last().unwrap() * original.rotation
                } else if let Some(k) = self
                    .input
                    .iter()
                    .position(|timestamp| *timestamp == current_time)
                {
                    output[k] * original.rotation
                } else {
                    let next = self
                        .input
                        .iter()
                        .position(|timestamp| *timestamp > current_time)
                        .unwrap();
                    let previous = next - 1;
                    match interpolation {
                        Interpolation::Linear => {
                            let t = (current_time - self.input[previous])
                                / (self.input[next] - self.input[previous]);
                            let dot = output[previous].dot(output[next]);
                            // dbg!(output[previous], output[previous].normalize());
                            let a = dot.abs().acos();
                            // dbg!(dot, dot.abs(), a);
                            let s = dot.signum();
                            let v_previous = Vec4::from_array(output[previous].to_array());
                            let v_next = Vec4::from_array(output[next].to_array());
                            let rotation = (a * (1.0 - t)).sin() / a.sin() * v_previous
                                + s * (a * t).sin() / a.sin() * v_next;
                            Quat::from_vec4(rotation).normalize()
                        }
                        Interpolation::Step => output[previous] * original.rotation,
                    }
                };

                let local_transform = Mat4::from_scale_rotation_translation(
                    transform.scale,
                    transform.rotation,
                    transform.translation,
                );

                set_node_local_transform(self.node, local_transform, nodes);
            }
            _ => todo!(),
        }
    }
}

#[derive(Debug)]
enum ChannelPath {
    Translation {
        original: TRS,
        interpolation: Interpolation,
        output: Vec<Vec3>,
    },
    Rotation {
        original: TRS,
        interpolation: Interpolation,
        output: Vec<Quat>,
    },
    Scale {
        original: TRS,
        interpolation: Interpolation,
        output: Vec<Vec3>,
    },
}

#[derive(Debug)]
enum Interpolation {
    Linear,
    Step,
}

#[derive(Debug, Clone, Copy)]
struct TRS {
    translation: Vec3,
    rotation: Quat,
    scale: Vec3,
}
