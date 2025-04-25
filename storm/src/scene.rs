use std::ops::{Index, IndexMut};

use crate::{
    math::Transform,
    storage::{DenseEntry, Id, SparseSet},
};

pub struct Scene {
    id: Id<Self>,
    pub name: String,
    nodes: SparseSet<Node>,
    root_nodes: Vec<Id<Node>>,
}

impl Scene {
    pub fn node_builder(&mut self) -> NodeBuilder {
        NodeBuilder::new(self)
    }

    pub fn root_nodes(&self) -> &[Id<Node>] {
        &self.root_nodes
    }
}

impl Index<Id<Node>> for Scene {
    type Output = Node;

    fn index(&self, index: Id<Node>) -> &Self::Output {
        &self.nodes[index]
    }
}

impl IndexMut<Id<Node>> for Scene {
    fn index_mut(&mut self, index: Id<Node>) -> &mut Self::Output {
        &mut self.nodes[index]
    }
}

pub struct SceneDescriptor {
    pub(super) name: Option<String>,
}

impl DenseEntry for Scene {
    type Key = Self;
    type Value = SceneDescriptor;

    fn new(id: Id<Self::Key>, desc: Self::Value) -> Self {
        let name = desc.name.unwrap_or_else(|| id.to_string());
        let nodes = SparseSet::new();
        let root_nodes = Vec::new();
        Scene {
            id,
            name,
            nodes,
            root_nodes,
        }
    }

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}

pub struct Node {
    id: Id<Node>,
    pub name: String,
    parent: Option<Id<Node>>,
    children: Vec<Id<Node>>,
    local_transform: Transform,
}

impl Node {
    pub fn children(&self) -> &[Id<Node>] {
        &self.children
    }

    pub fn local_transform(&self) -> &Transform {
        &self.local_transform
    }
}

pub struct NodeBuilder<'a> {
    scene: &'a mut Scene,
    desc: NodeDescriptor,
}

impl<'a> NodeBuilder<'a> {
    pub fn new(scene: &'a mut Scene) -> Self {
        Self {
            scene,
            desc: NodeDescriptor {
                parent: None,
                name: None,
                children: Vec::new(),
                local_transform: Transform::IDENTITY,
            },
        }
    }

    pub fn name(mut self, name: Option<String>) -> Self {
        self.desc.name = name;
        self
    }

    pub fn parent(mut self, parent: Option<Id<Node>>) -> Self {
        self.desc.parent = parent;
        self
    }

    pub fn build(self) -> &'a mut Node {
        let id = self.scene.nodes.next_id();
        match self.desc.parent {
            Some(parent) => self.scene.nodes[parent].children.push(id),
            None => self.scene.root_nodes.push(id),
        }
        self.scene.nodes.push(self.desc)
    }
}

pub struct NodeDescriptor {
    name: Option<String>,
    parent: Option<Id<Node>>,
    children: Vec<Id<Node>>,
    local_transform: Transform,
}

impl DenseEntry for Node {
    type Key = Self;
    type Value = NodeDescriptor;

    fn new(id: Id<Self::Key>, desc: Self::Value) -> Self {
        let name = desc.name.unwrap_or_else(|| id.to_string());
        Self {
            id,
            name,
            parent: desc.parent,
            children: desc.children,
            local_transform: desc.local_transform,
        }
    }

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}
