use std::{iter::FusedIterator, ops::Index};

use glam::{Mat4, Quat, Vec3};
use slotmap::{SlotMap, new_key_type};

/// A unique identifier for a node in the scene graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(NodeKey);

new_key_type! { struct NodeKey; }

/// A scene graph node. See [SceneGraph] for more information.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    parent: Option<NodeId>,
    children_count: usize,
    first_child: Option<NodeId>,
    /// the previous sibling in the list of children for the parent
    previous_sibling: NodeId,
    /// the next sibling in the list of children for the parent
    next_sibling: NodeId,

    local_translation: Vec3,
    local_rotation: Quat,
    local_scale: Vec3,

    global_transform: Mat4,
}

impl Node {
    /// Returns the parent's node id or `None` if the node is a root node.
    pub fn parent(&self) -> Option<NodeId> {
        self.parent
    }

    /// Returns the local translation of the node.
    /// See [`local_transform`][Self::local_transform] for more informations.
    pub fn local_translation(&self) -> Vec3 {
        self.local_translation
    }

    /// Returns the local rotation of the node.
    /// See [`local_transform`][Self::local_transform] for more informations.
    pub fn local_rotation(&self) -> Quat {
        self.local_rotation
    }

    /// Returns the local scale of the node.
    /// See [`local_transform`][Self::local_transform] for more informations.
    pub fn local_scale(&self) -> Vec3 {
        self.local_scale
    }

    /// Returns the local transformation of the node. The returns matrix can be used transform points
    /// from local space to the parent node's space. The local transformation is made up of
    /// the [local translation (`T`)][Self::local_translation], the [local rotation (`R`)][Self::local_rotation]
    /// and the [local scale (`S`)][Self::local_scale] in the `T * R * S` order (first the scale is applied to the point,
    /// then the rotation, and then the translation).
    /// When the node contains no parent, the local transformation is identical to the global transformation.
    pub fn local_transform(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            self.local_scale,
            self.local_rotation,
            self.local_translation,
        )
    }

    /// Returns the global transformation of the node. The returns matrix can be used transform points
    /// from local space to the global space. The global transformation of a node is the product of the
    /// global transformation matrix of its parent and its own [local transformation matrix][Self::local_transform].
    /// When the node contains no parent, the global transformation is identical to the local transformation.
    pub fn global_transform(&self) -> Mat4 {
        self.global_transform
    }
}

/// A Scene Graph works as a tree-like structure establishing parent-child relationships between scene elements,
/// creating logical groupings where transformations (position, rotation, scale) applied to parent nodes automatically
/// affect all their children - simplifying complex object manipulation and animation.
#[derive(Debug, Clone)]
pub struct SceneGraph {
    nodes: SlotMap<NodeKey, Node>,
    first_root: Option<NodeId>,
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

        let parent = parent.into();
        let node = Node {
            parent,
            children_count: 0,
            first_child: None,
            previous_sibling: NodeId(NodeKey::default()),
            next_sibling: NodeId(NodeKey::default()),
            local_translation,
            local_rotation,
            local_scale,
            global_transform: local_transform,
        };
        let id = NodeId(self.nodes.insert(node));

        let (global_transform, next_sibling) = match parent {
            Some(parent) => {
                let parent = &mut self.nodes[parent.0];
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
                let next = &mut self.nodes[next_sibling.0];
                let previous_sibling = next.previous_sibling;
                next.previous_sibling = id;
                self.nodes[previous_sibling.0].next_sibling = id;
                (previous_sibling, next_sibling)
            }
            None => {
                *next_sibling = Some(id);
                (id, id)
            }
        };

        let node = &mut self.nodes[id.0];
        node.global_transform = global_transform;
        node.previous_sibling = previous_sibling;
        node.next_sibling = next_sibling;

        id
    }

    /// Removes the node from the scene graph and returns it.
    /// Returns `None` if the node is not part of the scene graph.
    pub fn remove(&mut self, node: NodeId) -> Option<Node> {
        let id = node;
        let node = self.nodes.remove(id.0);
        if let Some(node) = &node {
            let parent = node.parent.map(|id| &mut self.nodes[id.0]);

            if node.next_sibling == id {
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

                self.nodes[node.previous_sibling.0].next_sibling = node.next_sibling;
                self.nodes[node.next_sibling.0].previous_sibling = node.previous_sibling;
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
                    let node = self.nodes.remove(state.next.0).unwrap();
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

    /// Returns `true` iff the scene graph contains the node.
    pub fn get(&self, node: NodeId) -> Option<&Node> {
        self.nodes.get(node.0)
    }

    /// Returns `true` iff the scene graph contains the node.
    pub fn contains(&self, node: NodeId) -> bool {
        self.nodes.contains_key(node.0)
    }

    /// Returns an iterator visiting all nodes in the scene graph once in arbitrary order.
    pub fn iter(&self) -> Iter<'_> {
        Iter {
            inner: self.nodes.iter(),
        }
    }

    /// Returns an iterator visiting all root nodes once in arbitrary order. A root node is a node with no parent.
    pub fn roots(&self) -> RootsIter<'_> {
        RootsIter {
            scene_graph: self,
            next: self.first_root,
            remaining: self.root_count,
        }
    }

    /// Returns n iterator visiting all parent nodes in bottom up order. The last elements will always be a root node.
    ///
    /// ## Panics
    ///
    /// Panics if the node is not part of the scene graph.
    pub fn parents(&self, node: NodeId) -> ParentsIter<'_> {
        ParentsIter {
            scene_graph: &self,
            next: self[node].parent,
        }
    }

    /// Returns an iterator visiting all sibling nodes once in arbitrary order. A sibling is a node with the same parent.
    ///
    /// ## Panics
    ///
    /// Panics if `node` is not part of the scene graph.
    pub fn siblings(&self, node: NodeId) -> SiblingsIter<'_> {
        SiblingsIter {
            scene_graph: self,
            first: node,
            next: self[node].next_sibling,
        }
    }

    /// Returns an iterator visiting all direct children nodes once in arbitrary order.
    ///
    /// ## Panics
    ///
    /// Panics if `node` is not part of the scene graph.
    pub fn direct_children(&self, node: NodeId) -> DirectChildrenIter<'_> {
        let parent = &self[node];
        let next = parent.first_child.unwrap_or(node);
        DirectChildrenIter {
            scene_graph: self,
            next,
            remaining: parent.children_count,
        }
    }

    /// Returns an iterator visition all children nodes once. Deeper children will always be visited after all their parent nodes. The order of siblings is arbitrary.
    ///
    /// ## Panics
    ///
    /// Panics if `node` is not part of the scene graph.
    pub fn all_children(&self, node: NodeId) -> AllChildrenIter<'_> {
        let parent = &self[node];
        let next = parent.first_child.unwrap_or(node);
        let states = vec![DepthIterState {
            next,
            remaining: parent.children_count,
        }];
        AllChildrenIter {
            scene_graph: self,
            states,
        }
    }

    pub fn try_set_local_transform(
        &mut self,
        node: NodeId,
        translation: impl Into<Option<Vec3>>,
        rotation: impl Into<Option<Quat>>,
        scale: impl Into<Option<Vec3>>,
    ) -> Result<(), ()> {
        let parent_transform = match self[node].parent {
            Some(parent) => self[parent].global_transform,
            None => Mat4::IDENTITY,
        };

        let node = &mut self.nodes[node.0];
        node.local_translation = translation.into().unwrap_or(node.local_translation);
        node.local_rotation = rotation.into().unwrap_or(node.local_rotation);
        node.local_scale = scale.into().unwrap_or(node.local_scale);
        node.global_transform = parent_transform * node.local_transform();

        // update children global_transform
        if let Some(next) = node.first_child {
            let mut states = vec![UpdateTransformIterState {
                next,
                remaining: node.children_count,
                parent_transform: node.global_transform,
            }];

            while let Some(state) = states.last_mut() {
                if state.remaining > 0 {
                    state.remaining -= 1;
                    let node = &mut self.nodes[state.next.0];
                    node.global_transform = state.parent_transform * node.local_transform();
                    state.next = node.next_sibling;
                    if let Some(next) = node.first_child {
                        states.push(UpdateTransformIterState {
                            next,
                            remaining: node.children_count,
                            parent_transform: node.global_transform,
                        });
                    }
                } else {
                    states.pop();
                }
            }
        }

        Ok(())
    }

    pub fn set_local_transform(
        &mut self,
        node: NodeId,
        translation: impl Into<Option<Vec3>>,
        rotation: impl Into<Option<Quat>>,
        scale: impl Into<Option<Vec3>>,
    ) {
        self.try_set_local_transform(node, translation, rotation, scale)
            .expect("entity should be part of the scene graph");
    }
}

impl Index<NodeId> for SceneGraph {
    type Output = Node;

    fn index(&self, index: NodeId) -> &Self::Output {
        &self.nodes[index.0]
    }
}

struct DeleteIterState {
    next: NodeId,
    remaining: usize,
}

struct UpdateTransformIterState {
    next: NodeId,
    remaining: usize,
    parent_transform: Mat4,
}

/// An iterator visiting all nodes in the scene graph once in arbitrary order.
#[derive(Debug, Clone)]
pub struct Iter<'a> {
    inner: slotmap::basic::Iter<'a, NodeKey, Node>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = (NodeId, &'a Node);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(key, node)| (NodeId(key), node))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a> ExactSizeIterator for Iter<'a> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a> FusedIterator for Iter<'a> {}

/// An iterator visiting all root nodes once in arbitrary order. A root node is a node with no parent.
#[derive(Debug, Clone)]
pub struct RootsIter<'a> {
    scene_graph: &'a SceneGraph,
    next: Option<NodeId>,
    remaining: usize,
}

impl<'a> Iterator for RootsIter<'a> {
    type Item = (NodeId, &'a Node);

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining > 0 {
            self.remaining -= 1;
            let id = self.next.unwrap();
            let node = &self.scene_graph[id];
            self.next = Some(node.next_sibling);
            Some((id, node))
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<'a> ExactSizeIterator for RootsIter<'a> {
    fn len(&self) -> usize {
        self.remaining
    }
}

impl<'a> FusedIterator for RootsIter<'a> {}

/// An iterator visiting all parent nodes in bottom up order. The last elements will always be a root node.
#[derive(Debug, Clone)]
pub struct ParentsIter<'a> {
    scene_graph: &'a SceneGraph,
    next: Option<NodeId>,
}

impl<'a> Iterator for ParentsIter<'a> {
    type Item = (NodeId, &'a Node);

    fn next(&mut self) -> Option<Self::Item> {
        self.next.map(|key| {
            let id = key;
            let node = &self.scene_graph[id];
            self.next = node.parent;
            (id, node)
        })
    }
}

impl<'a> FusedIterator for ParentsIter<'a> {}

/// An iterator visiting all sibling nodes once in arbitrary order. A sibling is a node with the same direct parent.
#[derive(Debug, Clone)]
pub struct SiblingsIter<'a> {
    scene_graph: &'a SceneGraph,
    first: NodeId,
    next: NodeId,
}

impl<'a> Iterator for SiblingsIter<'a> {
    type Item = (NodeId, &'a Node);

    fn next(&mut self) -> Option<Self::Item> {
        if self.next == self.first {
            None
        } else {
            let node = &self.scene_graph[self.next];
            let item = (self.next, node);
            self.next = node.next_sibling;
            Some(item)
        }
    }
}

impl<'a> FusedIterator for SiblingsIter<'a> {}

/// An iterator visiting all direct children nodes once in arbitrary order.
#[derive(Debug, Clone)]
pub struct DirectChildrenIter<'a> {
    scene_graph: &'a SceneGraph,
    next: NodeId,
    remaining: usize,
}

impl<'a> Iterator for DirectChildrenIter<'a> {
    type Item = (NodeId, &'a Node);

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining > 0 {
            self.remaining -= 1;
            let node = &self.scene_graph[self.next];
            let item = (self.next, node);
            self.next = node.next_sibling;
            Some(item)
        } else {
            None
        }
    }
}

impl<'a> ExactSizeIterator for DirectChildrenIter<'a> {
    fn len(&self) -> usize {
        self.remaining
    }
}

impl<'a> FusedIterator for DirectChildrenIter<'a> {}

/// An iterator visition all children nodes once. Deeper children will always be visited after all their parent nodes. The order of siblings is arbitrary.
#[derive(Debug, Clone)]
pub struct AllChildrenIter<'a> {
    scene_graph: &'a SceneGraph,
    states: Vec<DepthIterState>,
}

#[derive(Debug, Clone)]
struct DepthIterState {
    next: NodeId,
    remaining: usize,
}

impl<'a> Iterator for AllChildrenIter<'a> {
    type Item = (NodeId, &'a Node);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(state) = self.states.last_mut() {
            if state.remaining > 0 {
                state.remaining -= 1;
                let node = &self.scene_graph[state.next];
                let item = (state.next, node);
                state.next = node.next_sibling;
                if let Some(next) = node.first_child {
                    self.states.push(DepthIterState {
                        next,
                        remaining: node.children_count,
                    });
                }
                return Some(item);
            } else {
                self.states.pop();
            }
        }
        None
    }
}

impl<'a> FusedIterator for AllChildrenIter<'a> {}

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
        assert_eq!(Some(root_0), scene_graph.first_root);
        assert_eq!(root_0, scene_graph[root_0].previous_sibling);
        assert_eq!(root_0, scene_graph[root_0].next_sibling);

        let root_1 = scene_graph.add(None);
        assert_eq!(2, scene_graph.root_count);
        assert_eq!(Some(root_0), scene_graph.first_root);
        assert_eq!(root_1, scene_graph[root_0].previous_sibling);
        assert_eq!(root_0, scene_graph[root_1].previous_sibling);
        assert_eq!(root_1, scene_graph[root_0].next_sibling);
        assert_eq!(root_0, scene_graph[root_1].next_sibling);

        let root_2 = scene_graph.add(None);
        assert_eq!(3, scene_graph.root_count);
        assert_eq!(Some(root_0), scene_graph.first_root);
        assert_eq!(root_2, scene_graph[root_0].previous_sibling);
        assert_eq!(root_1, scene_graph[root_2].previous_sibling);
        assert_eq!(root_0, scene_graph[root_1].previous_sibling);
        assert_eq!(root_1, scene_graph[root_0].next_sibling);
        assert_eq!(root_2, scene_graph[root_1].next_sibling);
        assert_eq!(root_0, scene_graph[root_2].next_sibling);

        let result = scene_graph.remove(root_0);
        assert!(result.is_some());
        assert_eq!(2, scene_graph.root_count);
        assert_eq!(Some(root_1), scene_graph.first_root);
        assert_eq!(root_2, scene_graph[root_1].previous_sibling);
        assert_eq!(root_1, scene_graph[root_2].previous_sibling);
        assert_eq!(root_2, scene_graph[root_1].next_sibling);
        assert_eq!(root_1, scene_graph[root_2].next_sibling);

        let result = scene_graph.remove(root_2);
        assert!(result.is_some());
        assert_eq!(1, scene_graph.root_count);
        assert_eq!(Some(root_1), scene_graph.first_root);
        assert_eq!(root_1, scene_graph[root_1].previous_sibling);
        assert_eq!(root_1, scene_graph[root_1].next_sibling);

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
        assert_eq!(Some(child_0), scene_graph[root].first_child);
        assert_eq!(child_0, scene_graph[child_0].previous_sibling);
        assert_eq!(child_0, scene_graph[child_0].next_sibling);

        let child_1 = scene_graph.add(root);
        assert_eq!(2, scene_graph[root].children_count);
        assert_eq!(Some(child_0), scene_graph[root].first_child);
        assert_eq!(child_1, scene_graph[child_0].previous_sibling);
        assert_eq!(child_0, scene_graph[child_1].previous_sibling);
        assert_eq!(child_1, scene_graph[child_0].next_sibling);
        assert_eq!(child_0, scene_graph[child_1].next_sibling);

        let child_2 = scene_graph.add(root);
        assert_eq!(3, scene_graph[root].children_count);
        assert_eq!(Some(child_0), scene_graph[root].first_child);
        assert_eq!(child_2, scene_graph[child_0].previous_sibling);
        assert_eq!(child_1, scene_graph[child_2].previous_sibling);
        assert_eq!(child_0, scene_graph[child_1].previous_sibling);
        assert_eq!(child_1, scene_graph[child_0].next_sibling);
        assert_eq!(child_2, scene_graph[child_1].next_sibling);
        assert_eq!(child_0, scene_graph[child_2].next_sibling);

        let result = scene_graph.remove(child_0);
        assert!(result.is_some());
        assert_eq!(2, scene_graph[root].children_count);
        assert_eq!(Some(child_1), scene_graph[root].first_child);
        assert_eq!(child_2, scene_graph[child_1].previous_sibling);
        assert_eq!(child_1, scene_graph[child_2].previous_sibling);
        assert_eq!(child_2, scene_graph[child_1].next_sibling);
        assert_eq!(child_1, scene_graph[child_2].next_sibling);

        let result = scene_graph.remove(child_2);
        assert!(result.is_some());
        assert_eq!(1, scene_graph[root].children_count);
        assert_eq!(Some(child_1), scene_graph[root].first_child);
        assert_eq!(child_1, scene_graph[child_1].previous_sibling);
        assert_eq!(child_1, scene_graph[child_1].next_sibling);

        let result = scene_graph.remove(child_1);
        assert!(result.is_some());
        assert_eq!(0, scene_graph[root].children_count);
        assert_eq!(None, scene_graph[root].first_child);
    }

    #[test]
    fn test_scene_graph_iters() {
        let s = Setup::new();

        let mut iter = s.scene_graph.iter();
        assert_eq!(s.root_0, iter.next().unwrap().0);
        assert_eq!(s.child_0_0, iter.next().unwrap().0);
        assert_eq!(s.child_0_0_0, iter.next().unwrap().0);
        assert_eq!(s.root_1, iter.next().unwrap().0);
        assert_eq!(s.child_1_0, iter.next().unwrap().0);
        assert_eq!(s.child_1_0_0, iter.next().unwrap().0);
        assert_eq!(s.child_1_0_1, iter.next().unwrap().0);
        assert_eq!(s.child_1_1, iter.next().unwrap().0);
        assert!(iter.next().is_none());

        let mut iter = s.scene_graph.roots();
        assert_eq!(s.root_0, iter.next().unwrap().0);
        assert_eq!(s.root_1, iter.next().unwrap().0);
        assert!(iter.next().is_none());

        let mut iter = s.scene_graph.parents(s.child_0_0_0);
        assert_eq!(s.child_0_0, iter.next().unwrap().0);
        assert_eq!(s.root_0, iter.next().unwrap().0);
        assert!(iter.next().is_none());

        let mut iter = s.scene_graph.parents(s.child_1_1);
        assert_eq!(s.root_1, iter.next().unwrap().0);
        assert!(iter.next().is_none());

        let mut iter = s.scene_graph.siblings(s.root_0);
        assert_eq!(s.root_1, iter.next().unwrap().0);
        assert!(iter.next().is_none());

        let mut iter = s.scene_graph.siblings(s.child_0_0);
        assert!(iter.next().is_none());

        let mut iter = s.scene_graph.siblings(s.child_1_0);
        assert_eq!(s.child_1_1, iter.next().unwrap().0);
        assert!(iter.next().is_none());

        let mut iter = s.scene_graph.direct_children(s.root_0);
        assert_eq!(s.child_0_0, iter.next().unwrap().0);
        assert!(iter.next().is_none());

        let mut iter = s.scene_graph.direct_children(s.child_1_0);
        assert_eq!(s.child_1_0_0, iter.next().unwrap().0);
        assert_eq!(s.child_1_0_1, iter.next().unwrap().0);
        assert!(iter.next().is_none());

        let mut iter = s.scene_graph.all_children(s.root_0);
        assert_eq!(s.child_0_0, iter.next().unwrap().0);
        assert_eq!(s.child_0_0_0, iter.next().unwrap().0);
        assert!(iter.next().is_none());

        let mut iter = s.scene_graph.all_children(s.root_1);
        assert_eq!(s.child_1_0, iter.next().unwrap().0);
        assert_eq!(s.child_1_0_0, iter.next().unwrap().0);
        assert_eq!(s.child_1_0_1, iter.next().unwrap().0);
        assert_eq!(s.child_1_1, iter.next().unwrap().0);
        assert!(iter.next().is_none());

        let mut iter = s.scene_graph.all_children(s.child_1_0);
        assert_eq!(s.child_1_0_0, iter.next().unwrap().0);
        assert_eq!(s.child_1_0_1, iter.next().unwrap().0);
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_scene_graph_remove_children() {
        let mut s = Setup::new();

        s.scene_graph.remove(s.child_1_0);
        assert!(!s.scene_graph.contains(s.child_1_0));
        assert!(!s.scene_graph.contains(s.child_1_0_0));
        assert!(!s.scene_graph.contains(s.child_1_0_1));
        assert_eq!(5, s.scene_graph.iter().len());

        s.scene_graph.remove(s.root_1);
        assert!(!s.scene_graph.contains(s.root_1));
        assert!(!s.scene_graph.contains(s.child_1_1));
        assert_eq!(3, s.scene_graph.iter().len());

        s.scene_graph.remove(s.root_0);
        assert!(!s.scene_graph.contains(s.root_0));
        assert!(!s.scene_graph.contains(s.child_0_0));
        assert!(!s.scene_graph.contains(s.child_0_0_0));
        assert_eq!(0, s.scene_graph.iter().len());
    }

    #[test]
    #[rustfmt::skip]
    fn test_scene_graph_set_local_transform() {
        let mut s = Setup::new();

        assert_eq!(Mat4::IDENTITY, s.scene_graph[s.root_0].global_transform);
        assert_eq!(Mat4::IDENTITY, s.scene_graph[s.child_0_0].global_transform);
        assert_eq!(Mat4::IDENTITY, s.scene_graph[s.child_0_0_0].global_transform);
        assert_eq!(Mat4::IDENTITY, s.scene_graph[s.root_1].global_transform);
        assert_eq!(Mat4::IDENTITY, s.scene_graph[s.child_1_0].global_transform);
        assert_eq!(Mat4::IDENTITY, s.scene_graph[s.child_1_0_0].global_transform);
        assert_eq!(Mat4::IDENTITY, s.scene_graph[s.child_1_0_1].global_transform);
        assert_eq!(Mat4::IDENTITY, s.scene_graph[s.child_1_1].global_transform);

        let trans_1 = Mat4::from_scale_rotation_translation(Vec3::ONE, Quat::IDENTITY, Vec3::X);
        s.scene_graph.set_local_transform(s.root_0, Vec3::X, None, None);

        assert_eq!(trans_1, s.scene_graph[s.root_0].global_transform);
        assert_eq!(trans_1, s.scene_graph[s.child_0_0].global_transform);
        assert_eq!(trans_1, s.scene_graph[s.child_0_0_0].global_transform);
        assert_eq!(Mat4::IDENTITY, s.scene_graph[s.root_1].global_transform);
        assert_eq!(Mat4::IDENTITY, s.scene_graph[s.child_1_0].global_transform);
        assert_eq!(Mat4::IDENTITY, s.scene_graph[s.child_1_0_0].global_transform);
        assert_eq!(Mat4::IDENTITY, s.scene_graph[s.child_1_0_1].global_transform);
        assert_eq!(Mat4::IDENTITY, s.scene_graph[s.child_1_1].global_transform);

        let trans_2 = Mat4::from_scale_rotation_translation(Vec3::ONE, Quat::IDENTITY, Vec3::Y);
        s.scene_graph.set_local_transform(s.child_1_0, Vec3::Y, None, None);

        assert_eq!(trans_1, s.scene_graph[s.root_0].global_transform);
        assert_eq!(trans_1, s.scene_graph[s.child_0_0].global_transform);
        assert_eq!(trans_1, s.scene_graph[s.child_0_0_0].global_transform);
        assert_eq!(Mat4::IDENTITY, s.scene_graph[s.root_1].global_transform);
        assert_eq!(trans_2, s.scene_graph[s.child_1_0].global_transform);
        assert_eq!(trans_2, s.scene_graph[s.child_1_0_0].global_transform);
        assert_eq!(trans_2, s.scene_graph[s.child_1_0_1].global_transform);
        assert_eq!(Mat4::IDENTITY, s.scene_graph[s.child_1_1].global_transform);
    }
}
