use std::{collections::HashMap, time::Duration};

use glam::{Quat, Vec3};

use crate::asset::Asset;

use super::Scene;

pub struct AnimationManager {
    animations: Vec<Animation>,
}

impl AnimationManager {
    pub fn load(asset: &Asset, nodes_mapping: HashMap<usize, usize>) -> Self {
        let mut animations = Vec::with_capacity(asset.document.animations().len());
        asset.document.animations().for_each(|gltf_animation| {
            let mut channels = Vec::new();
            for gltf_channel in gltf_animation.channels() {
                let accessor = gltf_channel.sampler().input();
                let view = accessor.view().expect("sparse accessor are not supported");
                let start = view.offset() + accessor.offset();
                let end = start + accessor.count() * accessor.size();
                let input = bytemuck::cast_slice(&asset.buffers[view.buffer().index()][start..end])
                    .to_vec();

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
                            .to_vec();
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
            });
        });

        dbg!(&animations);

        Self { animations }
    }

    pub fn update(&mut self, delta_time: Duration, queue: &wgpu::Queue) {}
}

#[derive(Debug)]
struct TRS {
    translation: Vec3,
    rotation: Quat,
    scale: Vec3,
}

#[derive(Debug)]
struct Animation {
    channels: Vec<Channel>,
    current_time: f32,
}

#[derive(Debug)]
struct Channel {
    node: usize,
    path: ChannelPath,
    input: Vec<f32>,
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
