use std::sync::{Arc, Mutex};

use glam::{DVec3, Quat, U8Vec4, Vec3, dvec3, vec3};
use numpy::{PyArray1, PyArrayLike1};
use pyo3::{exceptions::PyValueError, prelude::*};
use tempete::{
    Context,
    ecs::{EntityId, EntityRegistry},
    geometry::Geometry,
    mesh::{MeshBuilder, MeshInstance, material::MaterialBuilder},
    scene_graph::SceneGraph,
};

use crate::PhysicsEngine;

const BASE_POS: Vec3 = vec3(0.0, 0.025, 0.8);
const BALL_RADIUS: f64 = 0.025;
const BALL_MASS: f64 = 0.170;
const GRAVITY: f64 = 9.81;

#[pyclass]
pub struct Ball {
    physics_id: tonner::BodyId,
    entity_id: EntityId,
    #[pyo3(get)]
    number: u8,
    #[pyo3(get)]
    pub radius: f64,
    #[pyo3(get, set)]
    pub out: bool,
    mesh_instance: MeshInstance,
    physics_engine: PhysicsEngine,
}

impl Ball {
    pub fn new<'py>(
        py: Python<'py>,
        number: u8,
        geometry: Geometry,
        name: String,
        color: impl Into<U8Vec4>,
        position: impl Into<Vec3>,
        velocity: impl Into<Vec3>,
        entity_registry: &mut EntityRegistry,
        scene_graph: Arc<Mutex<SceneGraph>>,
        physics_engine: PhysicsEngine,
        ctx: &Context,
    ) -> Bound<'py, Ball> {
        let position = position.into();
        let velocity = velocity.into();

        let mut engine = physics_engine.lock().unwrap();
        let physics_id = tonner::RigidBodyBuilder::default()
            .ball(tonner::shape::Ball::from_radius(BALL_RADIUS))
            .mass(BALL_MASS)
            .position(position.as_dvec3())
            .velocity(velocity.as_dvec3())
            .build(&mut engine);
        *engine.force_mut(physics_id).unwrap() = DVec3::NEG_Y * BALL_MASS * GRAVITY;
        drop(engine);

        let entity_id = entity_registry.create();

        scene_graph.lock().unwrap().add_with_transform(
            entity_id,
            None,
            position,
            Quat::IDENTITY,
            Vec3::ONE,
        );

        let material = MaterialBuilder::default()
            .name(name.clone())
            .base_color_factor(color.into().as_vec4() / 255.0)
            .metallic_factor(0.0)
            .build(ctx);

        let mesh_instance = MeshBuilder::default()
            .name(name)
            .primitive(geometry, material)
            .build(ctx)
            .unwrap()
            .new_instance(entity_id);

        let ball = Ball {
            physics_id,
            entity_id,
            number,
            radius: BALL_RADIUS,
            out: false,
            mesh_instance,
            physics_engine,
        };

        Bound::new(py, ball).unwrap()
    }

    pub fn physics_id(&self) -> tonner::BodyId {
        self.physics_id
    }

    pub fn entity_id(&self) -> EntityId {
        self.entity_id
    }

    pub fn mesh_instance(&self) -> &MeshInstance {
        &self.mesh_instance
    }
}

#[pymethods]
impl Ball {
    #[getter]
    fn position<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        let position = self
            .physics_engine
            .lock()
            .unwrap()
            .position(self.physics_id)
            .unwrap();
        PyArray1::from_slice(py, &position.to_array())
    }

    #[setter]
    fn set_position<'py>(&self, position: PyArrayLike1<'py, f64>) -> PyResult<()> {
        *self
            .physics_engine
            .lock()
            .unwrap()
            .position_mut(self.physics_id)
            .unwrap() = parse_dvec3(position)?;

        Ok(())
    }

    #[getter]
    fn velocity<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        let velocity = self
            .physics_engine
            .lock()
            .unwrap()
            .velocity(self.physics_id)
            .unwrap();
        PyArray1::from_slice(py, &velocity.to_array())
    }

    #[setter]
    fn set_velocity<'py>(&self, velocity: PyArrayLike1<'py, f64>) -> PyResult<()> {
        *self
            .physics_engine
            .lock()
            .unwrap()
            .velocity_mut(self.physics_id)
            .unwrap() = parse_dvec3(velocity)?;

        Ok(())
    }
}

fn parse_dvec3<'py>(value: PyArrayLike1<'py, f64>) -> PyResult<DVec3> {
    let array = value.as_array();
    if array.dim() != 3 {
        return Err(PyValueError::new_err("array's shape must be (3,)"));
    }
    Ok(dvec3(array[0], array[1], array[2]))
}

#[rustfmt::skip]
pub fn settings() -> Vec<(u8, &'static str, [u8; 4], Vec3, Vec3)> {
    // Standard spacing: dx = ball_diameter, dz = ball_diameter * sqrt(3)/2
    let d = 0.05; 
    let row_spacing = (3.0f32).sqrt() / 2.0 * d;

    vec![
        // white
        (0, "white", [255; 4], vec3(0.0, 0.025, -0.8), Vec3::ZERO),
        
        // Row 1
        (1, "solid yellow", [255, 217, 15, 255], BASE_POS + vec3(0.0, 0.0, 0.0), Vec3::ZERO),

        // Row 2
        (2, "solid blue", [5, 7, 255, 255], BASE_POS + vec3(row_spacing, 0.0, -0.5 * d), Vec3::ZERO),
        (9, "yellow stripe", [255, 217, 15, 255], BASE_POS + vec3(row_spacing, 0.0, 0.5 * d), Vec3::ZERO),

        // Row 3
        (3, "solid red", [255, 0, 0, 255], BASE_POS + vec3(2.0 * row_spacing, 0.0, -1.0 * d), Vec3::ZERO),
        (8, "solid black", [0, 0, 0, 255], BASE_POS + vec3(2.0 * row_spacing, 0.0, 0.0), Vec3::ZERO), 
        (10, "blue stripe", [5, 7, 255, 255], BASE_POS + vec3(2.0 * row_spacing, 0.0, 1.0 * d), Vec3::ZERO),

        // Row 4
        (4, "solid purple", [128, 0, 128, 255], BASE_POS + vec3(3.0 * row_spacing, 0.0, -1.5 * d), Vec3::ZERO),
        (11, "red stripe", [255, 0, 0, 255], BASE_POS + vec3(3.0 * row_spacing, 0.0, -0.5 * d), Vec3::ZERO),
        (5, "solid orange", [255, 165, 0, 255], BASE_POS + vec3(3.0 * row_spacing, 0.0, 0.5 * d), Vec3::ZERO),
        (12, "purple stripe", [128, 0, 128, 255], BASE_POS + vec3(3.0 * row_spacing, 0.0, 1.5 * d), Vec3::ZERO),

        // Row 5
        (6, "solid green", [0, 255, 0, 255], BASE_POS + vec3(4.0 * row_spacing, 0.0, -2.0 * d), Vec3::ZERO),
        (13, "orange stripe", [255, 165, 0, 255], BASE_POS + vec3(4.0 * row_spacing, 0.0, -1.0 * d), Vec3::ZERO),
        (7, "solid maroon", [128, 0, 0, 255], BASE_POS + vec3(4.0 * row_spacing, 0.0, 0.0), Vec3::ZERO),
        (14, "green stripe", [0, 255, 0, 255], BASE_POS + vec3(4.0 * row_spacing, 0.0, 1.0 * d), Vec3::ZERO),
        (15, "maroon stripe", [128, 0, 0, 255], BASE_POS + vec3(4.0 * row_spacing, 0.0, 2.0 * d), Vec3::ZERO),
    ]
}
