use crate::{NodeId, Scene};

#[must_use]
#[derive(Debug, Clone, Default)]
pub struct NodeBuilder {

}

impl NodeBuilder {
    pub fn build(self, scene: &mut Scene) -> NodeId {
        scene.scene_graph.add(None)
    }
}