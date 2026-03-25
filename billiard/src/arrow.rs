use pyo3::prelude::*;
use tonner::{
    Context,
    geometry::CylinderBuilder,
    mesh::{MeshBuilder, MeshInstance, material::MaterialBuilder},
    scene_graph::{NodeBuilder, PyNode, SceneGraph},
};

#[pyclass]
pub struct Arrow {
    #[pyo3(get)]
    node: Py<PyNode>,
    #[pyo3(get, set)]
    pub show: bool,
    mesh_instances: [MeshInstance; 1],
}

impl Arrow {
    pub fn new(py: Python, scene_graph: Py<SceneGraph>, ctx: &Context) -> Arrow {
        let node = NodeBuilder::default()
            .name("Arrow")
            .build(&mut scene_graph.borrow_mut(py))
            .unwrap();

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

        let arrow_node = NodeBuilder::default()
            .name("Arrow body")
            .parent(node)
            .build(&mut scene_graph.borrow_mut(py))
            .unwrap();

        let arrow_body = MeshBuilder::default()
            .name("Arrow body")
            .primitive(stick, black)
            .build(ctx)
            .unwrap()
            .new_instance(arrow_node);

        let node = Py::new(py, PyNode::new(node, scene_graph.clone_ref(py))).unwrap();

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
