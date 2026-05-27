use std::sync::{Arc, Mutex};

use glam::{Quat, U8Vec4, Vec3, vec3};
use numpy::{PyArray1, ToPyArray, ndarray::arr1};
use pyo3::prelude::*;
use tonner::{
    Context, ecs::EntityRegistry, geometry::Geometry, mesh::{MeshBuilder, MeshInstance, material::MaterialBuilder}, scene_graph::{NodeHandle, SceneGraph}
};

const BASE_POS: Vec3 = vec3(0.0, 0.025, 0.8);
const BALL_RADIUS: f64 = 0.025;

#[pyclass]
pub struct Ball {
    #[pyo3(get)]
    number: u8,
    #[pyo3(get)]
    node: Py<NodeHandle>,
    #[pyo3(get)]
    pub radius: f64,
    #[pyo3(get, set)]
    pub velocity: Py<PyArray1<f64>>,
    #[pyo3(get, set)]
    pub out: bool,
    mesh_instance: MeshInstance,
}

impl Ball {
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
        ctx: &Context,
    ) -> Bound<'py, Ball> {
        let entity = entity_registry.create();
        scene_graph.lock().unwrap().add_with_transform(entity, None, position.into(), Quat::IDENTITY, Vec3::ONE);

        let velocity = arr1(&velocity.into().to_array().map(|f| f as f64)).to_pyarray(py);

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
            .new_instance(entity);

        let ball = Ball {
            number,
            node: Py::new(py, NodeHandle::new(entity, scene_graph)).unwrap(),
            radius: BALL_RADIUS,
            velocity: velocity.into(),
            out: false,
            mesh_instance,
        };

        Bound::new(py, ball).unwrap()
    }

    pub fn node(&self) -> &Py<NodeHandle> {
        &self.node
    }

    pub fn mesh_instance(&self) -> &MeshInstance {
        &self.mesh_instance
    }
}
