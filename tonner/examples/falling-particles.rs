use std::time::Duration;

use tonner::{ParticleBuilder, Solver, State};
use glam::DVec3;

fn main() {
    env_logger::init();

    let mut state = State::new();

    let a = ParticleBuilder::default().mass(1.0).build(&mut state);
    let b = ParticleBuilder::default()
        .mass(1.0)
        .velocity(10.0 * DVec3::Z)
        .build(&mut state);

    let mut solver = Solver::default();
    let dt = Duration::from_secs(1);

    for body in [a, b] {
        *state.force_mut(body).unwrap() += 10.0 * DVec3::NEG_Z;
    }

    for _ in 0..10 {
        solver.simulate(&mut state, dt);
        println!("a: {}", state.position(a).unwrap());
        println!("b: {}", state.position(b).unwrap());
    }
}
