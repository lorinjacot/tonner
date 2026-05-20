use std::sync::{Arc, Mutex};

use pyo3::prelude::*;
use tonner::{
    Context,
    entity_component::EntityManager,
    geometry::CylinderBuilder,
    mesh::{MeshBuilder, MeshInstance, material::MaterialBuilder},
    scene_graph::{NodeHandle, SceneGraph},
};

#[pyclass]
pub struct Arrow {
    #[pyo3(get)]
    node: Py<NodeHandle>,
    #[pyo3(get, set)]
    pub show: bool,
    mesh_instances: [MeshInstance; 1],
}

impl Arrow {
    pub fn new(
        py: Python,
        entity_manager: &mut EntityManager,
        scene_graph: Arc<Mutex<SceneGraph>>,
        ctx: &Context,
    ) -> Arrow {
        let entity = entity_manager.new_entity();
        scene_graph.lock().unwrap().add(entity, None);

        let radius = 0.005;
        let stick = CylinderBuilder::default()
            .name("Arrow body")
            .height(1.0)
            .radius_top(radius)
            .radius_bottom(radius)
            .build(ctx);

        let black = MaterialBuilder::default()
            .name("Arrow material")
            .base_color_factor([0.0, 0.0, 0.0, 1.0])
            .metallic_factor(0.2)
            .build(ctx);

        let arrow_body = MeshBuilder::default()
            .name("Arrow body")
            .primitive(stick, black)
            .build(ctx)
            .unwrap()
            .new_instance(entity);

        let node = Py::new(py, NodeHandle::new(entity, scene_graph)).unwrap();

        Arrow {
            node,
            show: false,
            mesh_instances: [arrow_body],
        }
    }

    pub fn mesh_instances(&self) -> &[MeshInstance] {
        if self.show { &self.mesh_instances } else { &[] }
    }
}
