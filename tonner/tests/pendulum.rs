use std::time::Duration;

use glam::DVec3;
use tonner::{ParticleBuilder, Solver, State, constraint::particle::ParticleDistanceConstraint};

pub const L1: f64 = 1.0;
pub const L2: f64 = 1.0;
pub const M0: f64 = f64::INFINITY;
pub const M1: f64 = 1.0;
pub const M2: f64 = 1.0;
pub const G: f64 = 9.81;

pub fn theta1_ddot(theta1: f64, theta1_dot: f64, theta2: f64, theta2_dot: f64) -> f64 {
    let num = -G * (2.0 * M1 + M2) * theta1.sin()
        - M2 * G * (theta1 - 2.0 * theta2).sin()
        - 2.0
            * (theta1 - theta2).sin()
            * M2
            * (theta2_dot.powi(2) * L2 + theta1_dot.powi(2) * L1 * (theta1 - theta2).cos());
    let den = L1 * (2.0 * M1 + M2 - M2 * (2.0 * theta1 - 2.0 * theta2).cos());
    num / den
}

pub fn theta2_ddot(theta1: f64, theta1_dot: f64, theta2: f64, theta2_dot: f64) -> f64 {
    let num = 2.0
        * (theta1 - theta2).sin()
        * (theta1_dot.powi(2) * L1 * (M1 + M2)
            + G * (M1 + M2) * theta1.cos()
            + theta2_dot.powi(2) * L2 * M2 * (theta1 - theta2).cos());
    let den = L2 * (2.0 * M1 + M2 - M2 * (2.0 * theta1 - 2.0 * theta2).cos());
    num / den
}

#[test]
fn double_pendulum() {
    let mut state = State::new();
    let a = ParticleBuilder::default()
        .mass(M0)
        .position([0.0, 0.0, 0.0])
        .build(&mut state);
    let b = ParticleBuilder::default()
        .mass(M1)
        .position([L1, 0.0, 0.0])
        .build(&mut state);
    let c = ParticleBuilder::default()
        .mass(M2)
        .position([L1 + L2, 0.0, 0.0])
        .build(&mut state);

    state.add_particle_distance_constraint(ParticleDistanceConstraint {
        particles: [a, b],
        distance: L1,
        compliance: 0.0,
    });
    state.add_particle_distance_constraint(ParticleDistanceConstraint {
        particles: [b, c],
        distance: L2,
        compliance: 0.0,
    });

    state.force_mut(b).unwrap().y -= M1 * G;
    state.force_mut(c).unwrap().y -= M2 * G;

    let time_step = Duration::from_millis(10);
    let mut solver = Solver::default();

    let mut theta1 = std::f64::consts::FRAC_PI_2;
    let mut theta1_dot = 0.0;
    let mut theta2 = std::f64::consts::FRAC_PI_2;
    let mut theta2_dot = 0.0;

    for iteration in 0..100 {
        solver.simulate(&mut state, time_step);

        theta1_dot += theta1_ddot(theta1, theta1_dot, theta2, theta2_dot) * time_step.as_secs_f64();
        theta2_dot += theta2_ddot(theta1, theta1_dot, theta2, theta2_dot) * time_step.as_secs_f64();

        theta1 += theta1_dot * time_step.as_secs_f64();
        theta2 += theta2_dot * time_step.as_secs_f64();

        let expected_b_pos = DVec3::new(L1 * theta1.sin(), -L1 * theta1.cos(), 0.0);
        let expected_c_pos =
            expected_b_pos + DVec3::new(L2 * theta2.sin(), -L2 * theta2.cos(), 0.0);
        let actual_b_pos = state.position(b).unwrap();
        let actual_c_pos = state.position(c).unwrap();

        let max_abs_diff = 1e-2 + iteration as f64 * 1e-3;
        assert!(
            actual_b_pos.abs_diff_eq(expected_b_pos, max_abs_diff),
            "particle b: expected b at {:?}, got {:?} at iteration {}",
            expected_b_pos,
            actual_b_pos,
            iteration
        );
        assert!(
            actual_c_pos.abs_diff_eq(expected_c_pos, max_abs_diff),
            "particle c: expected c at {:?}, got {:?} at iteration {}",
            expected_c_pos,
            actual_c_pos,
            iteration
        );
    }
}
