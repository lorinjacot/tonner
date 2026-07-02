use std::time::Duration;

use glam::DVec3;
use tonner::{Engine, ParticleBuilder};

fn main() {
    env_logger::init();

    let mut engine = Engine::new();

    let a = ParticleBuilder::default().mass(1.0).build(&mut engine);
    let b = ParticleBuilder::default()
        .mass(1.0)
        .velocity(10.0 * DVec3::Z)
        .build(&mut engine);

    let dt = Duration::from_secs(1);

    for body in [a, b] {
        *engine.force_mut(body).unwrap() += 10.0 * DVec3::NEG_Z;
    }

    for _ in 0..10 {
        engine.simulate(dt);
        println!("a: {}", engine.position(a).unwrap());
        println!("b: {}", engine.position(b).unwrap());
    }
}
