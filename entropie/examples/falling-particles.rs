use std::time::Duration;

use entropie::{BodyId, ParticleBuilder, Solver, State, force::Force};
use glam::Vec3;

struct Gravity {
    bodies: [BodyId; 2],
    value: Vec3,
}

impl Force for Gravity {
    fn bodies(&self) -> &[BodyId] {
        &self.bodies
    }

    fn value(&self, _time: std::time::Duration) -> Vec3 {
        self.value
    }
}

fn main() {
    env_logger::init();

    let mut state = State::new();

    let a = ParticleBuilder::default().mass(1.0).build(&mut state);
    let b = ParticleBuilder::default()
        .mass(1.0)
        .velocity(10.0 * Vec3::Z)
        .build(&mut state);

    let gravity = Gravity {
        bodies: [a, b],
        value: 10.0 * Vec3::NEG_Z,
    };
    state.add_force(&gravity);

    let mut solver = Solver::default();
    let dt = Duration::from_secs(1);

    for _ in 0..10 {
        solver.simulate(&mut state, dt);
        println!("a: {}", state.position(a).unwrap());
        println!("b: {}", state.position(b).unwrap());
    }
}
