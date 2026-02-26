use glam::{U8Vec4, Vec3, vec3};
use numpy::{PyArray1, ToPyArray, ndarray::arr1};
use pyo3::prelude::*;
use storm::{
    Context,
    geometry::Geometry,
    mesh::{MeshBuilder, MeshInstance, material::MaterialBuilder},
    scene_graph::{NodeBuilder, PyNode, SceneGraph},
};

#[pyclass]
pub struct Ball {
    #[pyo3(get)]
    number: u8,
    #[pyo3(get)]
    node: Py<PyNode>,
    #[pyo3(get, set)]
    velocity: Py<PyArray1<f64>>,
}

impl Ball {
    #[rustfmt::skip]
    pub const NUMBER_NAME_COLOR_POSITION_VELOCITY: &'static [(u8, &'static str, [u8; 4], Vec3, Vec3)] =
        &[
            (1, "solid yellow", [255, 217, 15, 255], vec3(0.3, 0.025, 0.0), Vec3::ZERO),
            (1, "solid blue", [5, 7, 255, 255], vec3(0.15, 0.025, 0.0), Vec3::ZERO),
            (8, "solid back", [0, 0, 0, 255], vec3(0.0, 0.025, 0.0), vec3(0.3, 0.0, 0.0))
        ];

    pub fn new<'py>(
        py: Python<'py>,
        number: u8,
        geometry: Geometry,
        name: String,
        color: impl Into<U8Vec4>,
        position: impl Into<Vec3>,
        velocity: impl Into<Vec3>,
        scene_graph: Py<SceneGraph>,
        ctx: &Context,
    ) -> (Bound<'py, Ball>, MeshInstance) {
        let node_id = NodeBuilder::default()
            .name(name.clone())
            .local_translation(position)
            .build(&mut scene_graph.borrow_mut(py))
            .unwrap();

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
            .new_instance(node_id);

        let ball = Ball {
            number,
            node: Py::new(py, PyNode::new(node_id, scene_graph)).unwrap(),
            velocity: velocity.into(),
        };

        (Bound::new(py, ball).unwrap(), mesh_instance)
    }
}
