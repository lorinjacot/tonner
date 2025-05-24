use glam::{Quat, Vec3, Vec4};

use crate::{DenseEntry, Id, storage::SparseSet};

use super::Node;

struct Animation {
    id: Id<Self>,
    channels: Vec<Channel>,
}

impl Animation {
    pub(super) fn update(&self, current_timestamp: f32, nodes: &mut SparseSet<Node>) -> bool {
        self.channels
            .iter()
            .map(|channel| {
                let node = &mut nodes[channel.node];
                match &channel.ouput {
                    Output::Translations(slice) => {
                        let (translation, completed) = interpolate_vec3(
                            current_timestamp,
                            &channel.input,
                            channel.interpolation,
                            &slice,
                        );
                        node.local_transform.set_translation(translation);
                        completed
                    }
                    Output::Rotations(slice) => {
                        let (rotation, completed) = interpolate_quat(
                            current_timestamp,
                            &channel.input,
                            channel.interpolation,
                            &slice,
                        );
                        node.local_transform.set_rotation(rotation);
                        completed
                    }
                    Output::Scales(slice) => {
                        let (scale, completed) = interpolate_vec3(
                            current_timestamp,
                            &channel.input,
                            channel.interpolation,
                            &slice,
                        );
                        node.local_transform.set_scale(scale);
                        completed
                    }
                }
            })
            .all(|completed| completed)
    }
}

impl DenseEntry for Animation {
    type Key = Self;

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}

struct Channel {
    node: Id<Node>,
    input: Vec<f32>,
    interpolation: Interpolation,
    ouput: Output,
}

#[derive(Debug, Clone, Copy)]
enum Interpolation {
    Step,
    Linear,
    CubicSpline,
}

enum Output {
    Translations(Vec<[f32; 3]>),
    Rotations(Vec<[f32; 4]>),
    Scales(Vec<[f32; 3]>),
}

fn interpolate_vec3(
    current_timestamp: f32,
    inputs: &[f32],
    interpolation: Interpolation,
    ouputs: &[[f32; 3]],
) -> (Vec3, bool) {
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
) -> (Quat, bool) {
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
) -> (T, bool) {
    let t_c = current_timestamp;
    let mut iter = inputs.iter().enumerate();
    while let Some((k, t_k)) = iter.next() {
        if t_c == *t_k {
            return (step_callback(&ouputs[k]), false);
        } else if t_c < *t_k {
            if k == 0 {
                return (step_callback(ouputs.first().unwrap()), false);
            } else {
                return match interpolation {
                    Interpolation::Step => (step_callback(&ouputs[k - 1]), false),
                    Interpolation::Linear => {
                        let t_previous = inputs[k - 1];
                        let v_previous = &ouputs[k - 1];
                        let t_next = *t_k;
                        let v_next = &ouputs[k];
                        let t_d = t_next - t_previous;
                        let t = (t_c - t_k) / t_d;
                        (linear_callback(t, v_previous, v_next), false)
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

                        (
                            cubic_spline_callback(t, t_d, a_next, v_prev, v_next, b_prev),
                            false,
                        )
                    }
                };
            }
        }
    }
    (step_callback(ouputs.last().unwrap()), true)
}
