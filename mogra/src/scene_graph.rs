use std::ops::Index;

use glam::{Mat4, Quat, Vec3};
use slotmap::{SlotMap, new_key_type};

/// A Scene Graph works as a tree-like structure establishing parent-child relationships between scene elements,
/// creating logical groupings where transformations (position, rotation, scale) applied to parent nodes automatically
/// affect all their children - simplifying complex object manipulation and animation.
#[derive(Debug, Clone)]
pub struct SceneGraph {
    nodes: SlotMap<NodeKey, Node>,
    first_root: Option<NodeKey>,
    root_count: usize,
}

impl SceneGraph {
    /// Constructs a new, empty scene graph.
    ///
    /// The scene graph will not allocate until nodes are added.
    pub(super) fn new() -> Self {
        Self {
            nodes: SlotMap::with_key(),
            first_root: None,
            root_count: 0,
        }
    }

    /// Create a new node in the scene graph and returns its unique id.
    /// If `parent` is `None`, the new node will be a root node.
    /// Otherwise, the new node will be a child of `parent`.
    ///
    /// The new node will have a local translation of `Vec3::ZERO`, a local rotation of `Quat::IDENTITY` and a local scale of `Vec3::ONE`.
    ///
    /// ## Panics
    ///
    /// Panics if `parent` is not part of the scene graph.
    pub fn add(&mut self, parent: impl Into<Option<NodeId>>) -> NodeId {
        self.add_with_transform(parent, None, None, None)
    }

    /// Create a new node in the scene graph and returns its unique id.
    /// If `parent` is `None`, the new node will be a root node.
    /// Otherwise, the new node will be a child of `parent`.
    ///
    /// If `local_translation`, `local_rotation` or `local_scale` are `None`, they will default to `Vec3::ZERO`, `Quat::IDENTITY` and `Vec3::ONE` respectively.
    ///
    /// ## Panics
    ///
    /// Panics if `parent` is not part of the scene graph.
    pub fn add_with_transform(
        &mut self,
        parent: impl Into<Option<NodeId>>,
        local_translation: impl Into<Option<Vec3>>,
        local_rotation: impl Into<Option<Quat>>,
        local_scale: impl Into<Option<Vec3>>,
    ) -> NodeId {
        let local_translation = local_translation.into().unwrap_or(Vec3::ZERO);
        let local_rotation = local_rotation.into().unwrap_or(Quat::IDENTITY);
        let local_scale = local_scale.into().unwrap_or(Vec3::ONE);
        let local_transform =
            Mat4::from_scale_rotation_translation(local_scale, local_rotation, local_translation);

        let parent = parent.into().map(|id| id.0);
        let node = Node {
            parent,
            children_count: 0,
            first_child: None,
            previous_sibling: NodeKey::default(),
            next_sibling: NodeKey::default(),
            local_translation,
            local_rotation,
            local_scale,
            global_transform: local_transform,
        };
        let key = self.nodes.insert(node);

        let (global_transform, next_sibling) = match parent {
            Some(parent) => {
                let parent = &mut self.nodes[parent];
                parent.children_count += 1;
                (
                    parent.global_transform * local_transform,
                    &mut parent.first_child,
                )
            }
            None => {
                self.root_count += 1;
                (local_transform, &mut self.first_root)
            }
        };

        let (previous_sibling, next_sibling) = match *next_sibling {
            Some(next_sibling) => {
                // parent already contains another child -> patch double linked list
                let next = &mut self.nodes[next_sibling];
                let previous_sibling = next.previous_sibling;
                next.previous_sibling = key;
                self.nodes[previous_sibling].next_sibling = key;
                (previous_sibling, next_sibling)
            }
            None => {
                *next_sibling = Some(key);
                (key, key)
            }
        };

        let node = &mut self.nodes[key];
        node.global_transform = global_transform;
        node.previous_sibling = previous_sibling;
        node.next_sibling = next_sibling;

        NodeId(key)
    }

    /// Removes the node from the scene graph and returns it.
    /// Returns `None` if the node is not part of the scene graph.
    pub fn remove(&mut self, node: NodeId) -> Option<Node> {
        let key = node.0;
        let node = self.nodes.remove(key);
        if let Some(node) = &node {
            let parent = node.parent.map(|id| &mut self.nodes[id]);

            if node.next_sibling == key {
                if let Some(parent) = parent {
                    parent.children_count = 0;
                    parent.first_child = None;
                } else {
                    self.root_count = 0;
                    self.first_root = None;
                }
            } else {
                if let Some(parent) = parent {
                    parent.children_count -= 1;
                    parent.first_child = Some(node.next_sibling);
                } else {
                    self.root_count -= 1;
                    self.first_root = Some(node.next_sibling);
                }

                self.nodes[node.previous_sibling].next_sibling = node.next_sibling;
                self.nodes[node.next_sibling].previous_sibling = node.previous_sibling;
            }

            self.remove_children(node);
        }
        node
    }

    fn remove_children(&mut self, node: &Node) {
        if let Some(next) = node.first_child {
            let mut states = vec![DeleteIterState {
                next,
                remaining: node.children_count,
            }];

            while let Some(state) = states.last_mut() {
                if state.remaining > 0 {
                    state.remaining -= 1;
                    let node = self.nodes.remove(state.next).unwrap();
                    state.next = node.next_sibling;
                    if let Some(next) = node.first_child {
                        states.push(DeleteIterState {
                            next,
                            remaining: node.children_count,
                        });
                    }
                } else {
                    states.pop();
                }
            }
        }
    }

    pub fn get(&self, node: NodeId) -> Option<&Node> {
        self.nodes.get(node.0)
    }
}

impl Index<NodeId> for SceneGraph {
    type Output = Node;

    fn index(&self, index: NodeId) -> &Self::Output {
        &self.nodes[index.0]
    }
}

/// A unique identifier for a node in the scene graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(NodeKey);

new_key_type! { struct NodeKey; }

/// A scene graph node. See [SceneGraph] for more information.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    parent: Option<NodeKey>,
    children_count: usize,
    first_child: Option<NodeKey>,
    /// the previous sibling in the list of children for the parent
    previous_sibling: NodeKey,
    /// the next sibling in the list of children for the parent
    next_sibling: NodeKey,

    local_translation: Vec3,
    local_rotation: Quat,
    local_scale: Vec3,

    global_transform: Mat4,
}

struct DeleteIterState {
    next: NodeKey,
    remaining: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Setup {
        scene_graph: SceneGraph,
        root_0: NodeId,
        child_0_0: NodeId,
        child_0_0_0: NodeId,
        root_1: NodeId,
        child_1_0: NodeId,
        child_1_0_0: NodeId,
        child_1_0_1: NodeId,
        child_1_1: NodeId,
    }

    impl Setup {
        fn new() -> Setup {
            let mut scene_graph = SceneGraph::new();

            let root_0 = scene_graph.add(None);
            let child_0_0 = scene_graph.add(root_0);
            let child_0_0_0 = scene_graph.add(child_0_0);

            let root_1 = scene_graph.add(None);
            let child_1_0 = scene_graph.add(root_1);
            let child_1_0_0 = scene_graph.add(child_1_0);
            let child_1_0_1 = scene_graph.add(child_1_0);
            let child_1_1 = scene_graph.add(root_1);

            Setup {
                scene_graph,
                root_0,
                child_0_0,
                child_0_0_0,
                root_1,
                child_1_0,
                child_1_0_0,
                child_1_0_1,
                child_1_1,
            }
        }
    }

    #[test]
    fn test_scene_graph_roots() {
        let mut scene_graph = SceneGraph::new();

        let root_0 = scene_graph.add(None);
        assert_eq!(1, scene_graph.root_count);
        assert_eq!(Some(root_0.0), scene_graph.first_root);
        assert_eq!(root_0.0, scene_graph[root_0].previous_sibling);
        assert_eq!(root_0.0, scene_graph[root_0].next_sibling);

        let root_1 = scene_graph.add(None);
        assert_eq!(2, scene_graph.root_count);
        assert_eq!(Some(root_0.0), scene_graph.first_root);
        assert_eq!(root_1.0, scene_graph[root_0].previous_sibling);
        assert_eq!(root_0.0, scene_graph[root_1].previous_sibling);
        assert_eq!(root_1.0, scene_graph[root_0].next_sibling);
        assert_eq!(root_0.0, scene_graph[root_1].next_sibling);

        let root_2 = scene_graph.add(None);
        assert_eq!(3, scene_graph.root_count);
        assert_eq!(Some(root_0.0), scene_graph.first_root);
        assert_eq!(root_2.0, scene_graph[root_0].previous_sibling);
        assert_eq!(root_1.0, scene_graph[root_2].previous_sibling);
        assert_eq!(root_0.0, scene_graph[root_1].previous_sibling);
        assert_eq!(root_1.0, scene_graph[root_0].next_sibling);
        assert_eq!(root_2.0, scene_graph[root_1].next_sibling);
        assert_eq!(root_0.0, scene_graph[root_2].next_sibling);

        let result = scene_graph.remove(root_0);
        assert!(result.is_some());
        assert_eq!(2, scene_graph.root_count);
        assert_eq!(Some(root_1.0), scene_graph.first_root);
        assert_eq!(root_2.0, scene_graph[root_1].previous_sibling);
        assert_eq!(root_1.0, scene_graph[root_2].previous_sibling);
        assert_eq!(root_2.0, scene_graph[root_1].next_sibling);
        assert_eq!(root_1.0, scene_graph[root_2].next_sibling);

        let result = scene_graph.remove(root_2);
        assert!(result.is_some());
        assert_eq!(1, scene_graph.root_count);
        assert_eq!(Some(root_1.0), scene_graph.first_root);
        assert_eq!(root_1.0, scene_graph[root_1].previous_sibling);
        assert_eq!(root_1.0, scene_graph[root_1].next_sibling);

        let result = scene_graph.remove(root_1);
        assert!(result.is_some());
        assert_eq!(0, scene_graph.root_count);
        assert_eq!(None, scene_graph.first_root);
    }

    #[test]
    fn test_scene_graph_children() {
        let mut scene_graph = SceneGraph::new();

        let root = scene_graph.add(None);

        assert_eq!(0, scene_graph[root].children_count);
        assert_eq!(None, scene_graph[root].first_child);

        let child_0 = scene_graph.add(root);
        assert_eq!(1, scene_graph[root].children_count);
        assert_eq!(Some(child_0.0), scene_graph[root].first_child);
        assert_eq!(child_0.0, scene_graph[child_0].previous_sibling);
        assert_eq!(child_0.0, scene_graph[child_0].next_sibling);

        let child_1 = scene_graph.add(root);
        assert_eq!(2, scene_graph[root].children_count);
        assert_eq!(Some(child_0.0), scene_graph[root].first_child);
        assert_eq!(child_1.0, scene_graph[child_0].previous_sibling);
        assert_eq!(child_0.0, scene_graph[child_1].previous_sibling);
        assert_eq!(child_1.0, scene_graph[child_0].next_sibling);
        assert_eq!(child_0.0, scene_graph[child_1].next_sibling);

        let child_2 = scene_graph.add(root);
        assert_eq!(3, scene_graph[root].children_count);
        assert_eq!(Some(child_0.0), scene_graph[root].first_child);
        assert_eq!(child_2.0, scene_graph[child_0].previous_sibling);
        assert_eq!(child_1.0, scene_graph[child_2].previous_sibling);
        assert_eq!(child_0.0, scene_graph[child_1].previous_sibling);
        assert_eq!(child_1.0, scene_graph[child_0].next_sibling);
        assert_eq!(child_2.0, scene_graph[child_1].next_sibling);
        assert_eq!(child_0.0, scene_graph[child_2].next_sibling);

        let result = scene_graph.remove(child_0);
        assert!(result.is_some());
        assert_eq!(2, scene_graph[root].children_count);
        assert_eq!(Some(child_1.0), scene_graph[root].first_child);
        assert_eq!(child_2.0, scene_graph[child_1].previous_sibling);
        assert_eq!(child_1.0, scene_graph[child_2].previous_sibling);
        assert_eq!(child_2.0, scene_graph[child_1].next_sibling);
        assert_eq!(child_1.0, scene_graph[child_2].next_sibling);

        let result = scene_graph.remove(child_2);
        assert!(result.is_some());
        assert_eq!(1, scene_graph[root].children_count);
        assert_eq!(Some(child_1.0), scene_graph[root].first_child);
        assert_eq!(child_1.0, scene_graph[child_1].previous_sibling);
        assert_eq!(child_1.0, scene_graph[child_1].next_sibling);

        let result = scene_graph.remove(child_1);
        assert!(result.is_some());
        assert_eq!(0, scene_graph[root].children_count);
        assert_eq!(None, scene_graph[root].first_child);
    }
}
