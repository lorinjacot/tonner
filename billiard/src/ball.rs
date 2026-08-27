use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use glam::{DVec3, Quat, Vec3, dvec3, vec3};
use numpy::{PyArray1, PyArrayLike1};
use pyo3::{exceptions::PyValueError, prelude::*};
use tempete::{
    Context,
    ecs::{EntityId, EntityRegistry},
    mesh::{Mesh, MeshInstance},
    scene_graph::SceneGraph,
};

use crate::PhysicsEngine;

const ASSET_PATH: &'static str = "assets/balls/scene.gltf";

const BASE_POS: Vec3 = vec3(0.0, 0.025, 0.8);
const BALL_RADIUS: f64 = 0.018931598663330078;
const BALL_MASS: f64 = 0.170;
const GRAVITY: f64 = 9.81;

/// The color of a billiard ball, including the white cue ball, solid balls, striped balls, and the black 8-ball.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BallColor {
    White = 0,
    SolidYellow = 1,
    SolidBlue = 2,
    SolidRed = 3,
    SolidPurple = 4,
    SolidOrange = 5,
    SolidGreen = 6,
    SolidMaroon = 7,
    Black = 8,
    YellowStripe = 9,
    BlueStripe = 10,
    RedStripe = 11,
    PurpleStripe = 12,
    OrangeStripe = 13,
    GreenStripe = 14,
    MaroonStripe = 15,
}

impl BallColor {
    /// The number of distinct ball colors, including the white cue ball.
    pub const COUNT: usize = 16;

    /// Returns the number of the ball color.
    ///
    /// The white cue ball has number 0, solid balls have numbers 1-7, the black 8-ball has number 8, and striped balls have numbers 9-15.
    pub fn number(&self) -> u8 {
        *self as u8
    }

    /// Returns the `BallColor` corresponding to the given number, or `None` if the number is invalid (i.e., not in the range 0-15).
    pub fn from_number(number: u8) -> Option<BallColor> {
        match number {
            0 => Some(BallColor::White),
            1 => Some(BallColor::SolidYellow),
            2 => Some(BallColor::SolidBlue),
            3 => Some(BallColor::SolidRed),
            4 => Some(BallColor::SolidPurple),
            5 => Some(BallColor::SolidOrange),
            6 => Some(BallColor::SolidGreen),
            7 => Some(BallColor::SolidMaroon),
            8 => Some(BallColor::Black),
            9 => Some(BallColor::YellowStripe),
            10 => Some(BallColor::BlueStripe),
            11 => Some(BallColor::RedStripe),
            12 => Some(BallColor::PurpleStripe),
            13 => Some(BallColor::OrangeStripe),
            14 => Some(BallColor::GreenStripe),
            15 => Some(BallColor::MaroonStripe),
            _ => None,
        }
    }

    fn from_asset_name(name: &str) -> Option<BallColor> {
        match name {
            "Ball Clube_10 - Default_0" => Some(BallColor::White),
            "Ball1_01 - Default_0" => Some(BallColor::SolidYellow),
            "Ball2_02 - Default_0" => Some(BallColor::SolidBlue),
            "Ball3_03 - Default_0" => Some(BallColor::SolidRed),
            "Ball4_07 - Default_0" => Some(BallColor::SolidPurple),
            "Ball5_08 - Default_0" => Some(BallColor::SolidOrange),
            "Ball6_09 - Default_0" => Some(BallColor::SolidGreen),
            "Ball7_13 - Default_0" => Some(BallColor::SolidMaroon),
            "Ball8_14 - Default_0" => Some(BallColor::Black),
            "Ball9_15 - Default_0" => Some(BallColor::YellowStripe),
            "Ball10_19 - Default_0" => Some(BallColor::BlueStripe),
            "Ball11_20 - Default_0" => Some(BallColor::RedStripe),
            "Ball12_21 - Default_0" => Some(BallColor::PurpleStripe),
            "Ball13_04 - Default_0" => Some(BallColor::OrangeStripe),
            "Ball14_05 - Default_0" => Some(BallColor::GreenStripe),
            "Ball15_06 - Default_0" => Some(BallColor::MaroonStripe),
            _ => None,
        }
    }
}

/// A collection of meshes for billiard balls, indexed by their color.
pub struct BallsAsset {
    meshes_by_color: HashMap<BallColor, Mesh>,
}

impl BallsAsset {
    pub fn load(ctx: &Context, encoder: &mut wgpu::CommandEncoder) -> anyhow::Result<BallsAsset> {
        let mut asset = storm_gltf::GltfAsset::open(ASSET_PATH)?;

        let meshes = asset.load_meshes(ctx, encoder)?;
        let meshes_by_color: HashMap<BallColor, Mesh> = meshes
            .into_iter()
            .filter_map(|mesh| {
                let color = BallColor::from_asset_name(&mesh.name())?;
                Some((color, mesh))
            })
            .collect();

        if meshes_by_color.len() != BallColor::COUNT {
            anyhow::bail!(
                "Expected {} ball meshes, but found {}",
                BallColor::COUNT,
                meshes_by_color.len()
            );
        }

        Ok(BallsAsset { meshes_by_color })
    }

    /// Returns the mesh corresponding to the given ball color.
    pub fn get(&self, color: BallColor) -> &Mesh {
        &self.meshes_by_color[&color]
    }
}

#[pyclass]
pub struct Ball {
    physics_id: tonner::BodyId,
    entity_id: EntityId,
    color: BallColor,
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
        color: BallColor,
        position: impl Into<Vec3>,
        velocity: impl Into<Vec3>,
        entity_registry: &mut EntityRegistry,
        scene_graph: Arc<Mutex<SceneGraph>>,
        physics_engine: PhysicsEngine,
        balls_assets: &BallsAsset,
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
            Vec3::splat(1e-3),
        );

        let mesh_instance = balls_assets.get(color).new_instance(entity_id);

        let ball = Ball {
            physics_id,
            entity_id,
            color,
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
    fn number(&self) -> u8 {
        self.color.number()
    }

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
pub fn settings() -> Vec<(BallColor, Vec3, Vec3)> {
    // Standard spacing: dx = ball_diameter, dz = ball_diameter * sqrt(3)/2
    let d = 0.05; 
    let row_spacing = (3.0f32).sqrt() / 2.0 * d;

    vec![
        // white
        (BallColor::White, vec3(0.0, 0.025, -0.8), Vec3::ZERO),
        
        // Row 1
        (BallColor::SolidYellow, BASE_POS + vec3(0.0, 0.0, 0.0), Vec3::ZERO),

        // Row 2
        (BallColor::SolidBlue, BASE_POS + vec3(row_spacing, 0.0, -0.5 * d), Vec3::ZERO),
        (BallColor::YellowStripe, BASE_POS + vec3(row_spacing, 0.0, 0.5 * d), Vec3::ZERO),

        // Row 3
        (BallColor::SolidRed, BASE_POS + vec3(2.0 * row_spacing, 0.0, -1.0 * d), Vec3::ZERO),
        (BallColor::Black, BASE_POS + vec3(2.0 * row_spacing, 0.0, 0.0), Vec3::ZERO), 
        (BallColor::BlueStripe, BASE_POS + vec3(2.0 * row_spacing, 0.0, 1.0 * d), Vec3::ZERO),

        // Row 4
        (BallColor::SolidPurple, BASE_POS + vec3(3.0 * row_spacing, 0.0, -1.5 * d), Vec3::ZERO),
        (BallColor::RedStripe, BASE_POS + vec3(3.0 * row_spacing, 0.0, -0.5 * d), Vec3::ZERO),
        (BallColor::SolidOrange, BASE_POS + vec3(3.0 * row_spacing, 0.0, 0.5 * d), Vec3::ZERO),
        (BallColor::PurpleStripe, BASE_POS + vec3(3.0 * row_spacing, 0.0, 1.5 * d), Vec3::ZERO),

        // Row 5
        (BallColor::SolidGreen, BASE_POS + vec3(4.0 * row_spacing, 0.0, -2.0 * d), Vec3::ZERO),
        (BallColor::OrangeStripe, BASE_POS + vec3(4.0 * row_spacing, 0.0, -1.0 * d), Vec3::ZERO),
        (BallColor::SolidMaroon, BASE_POS + vec3(4.0 * row_spacing, 0.0, 0.0), Vec3::ZERO),
        (BallColor::GreenStripe, BASE_POS + vec3(4.0 * row_spacing, 0.0, 1.0 * d), Vec3::ZERO),
        (BallColor::MaroonStripe, BASE_POS + vec3(4.0 * row_spacing, 0.0, 2.0 * d), Vec3::ZERO),
    ]
}
