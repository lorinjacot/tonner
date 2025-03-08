use std::collections::HashMap;

use glam::Mat4;

use crate::storage::{Id, Storage};

use super::mesh::MeshId;

pub struct Scene {
    nodes: Storage<Node>,
    mesh_nodes: HashMap<MeshId, Vec<NodeId>>,
}

pub type NodeId = Id<Node>;

pub struct Node {
    local_transform: Mat4,
    global_tansform: Mat4,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
}