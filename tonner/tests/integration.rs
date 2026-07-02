use std::time::Duration;

use glam::dvec3;
use tonner::{ParticleBuilder, Solver, State};

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

    let mut state = State::new();
    let body = ParticleBuilder::default()
        .position(p0)
        .velocity(v0)
        .inverse_mass(1.0)
        .build(&mut state);
    *state.force_mut(body).unwrap() = f;

    let mut solver = Solver::default();
    solver.substep_count = 1;
    for (i, expected_pos) in expected.into_iter().enumerate() {
        solver.simulate(&mut state, DELTA_TIME);
        let actual_pos = state.position(body).unwrap();
        assert!(
            actual_pos.abs_diff_eq(expected_pos, 1e-4),
            "Iteration {}: expected {:?}, got {:?}",
            i,
            expected_pos,
            actual_pos
        );
    }
}
