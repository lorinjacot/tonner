use std::time::Duration;

use glam::{DMat3, DQuat, DVec3, dvec3};
use tonner::{Engine, ParticleBuilder, RigidBodyBuilder, shape::Box3D};

#[test]
fn implicit_euler() {
    const ITERATOR_COUNT: usize = 10;
    const DELTA_TIME: Duration = Duration::from_millis(1);
    const DT: f64 = DELTA_TIME.as_secs_f64();

    let p0 = dvec3(1.0, 2.0, 3.0);
    let v0 = dvec3(10.0, 20.0, 30.0);
    let f = dvec3(100.0, 200.0, 300.0);
    let expected: Vec<_> = (0..ITERATOR_COUNT)
        .scan((p0, v0), |(p, v), _| {
            let a = f; // mass = 1.0
            *v += a * DT;
            *p += *v * DT;
            Some(*p)
        })
        .collect();

    let mut engine = Engine::new();
    let body = ParticleBuilder::default()
        .position(p0)
        .velocity(v0)
        .inverse_mass(1.0)
        .build(&mut engine);
    *engine.force_mut(body).unwrap() = f;

    engine.set_substep_count(1);
    for (i, expected_pos) in expected.into_iter().enumerate() {
        engine.simulate(DELTA_TIME);
        let actual_pos = engine.position(body).unwrap();
        assert!(
            actual_pos.abs_diff_eq(expected_pos, 1e-4),
            "Iteration {}: expected {:?}, got {:?}",
            i,
            expected_pos,
            actual_pos
        );
    }
}

#[test]
fn rotation() {
    let mut engine = Engine::new();
    let body = RigidBodyBuilder::default()
        .mass(1.0)
        .inertia(DMat3::IDENTITY)
        .angular_velocity([0.0, 1.0, 0.0])
        .box3d(Box3D::from_dimensions(1.0, 1.0, 1.0))
        .build(&mut engine);

    let time_step = Duration::from_millis(100);

    for iteration in 0..100 {
        engine.simulate(time_step);
        let orientation = engine.orientation(body).unwrap();
        let expected_angle = (iteration + 1) as f64 * time_step.as_secs_f64();
        let expected_orientation = DQuat::from_axis_angle(DVec3::Y, expected_angle);
        let max_abs_diff = 1e-2 + iteration as f64 * 1e-3;
        assert!(
            orientation.abs_diff_eq(expected_orientation, max_abs_diff),
            "expected orientation {:?}, got {:?} at iteration {}",
            expected_orientation,
            orientation,
            iteration
        );
    }
}
