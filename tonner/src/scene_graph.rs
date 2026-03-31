use std::{iter::FusedIterator, ops::Index};

use glam::{Mat4, Quat, Vec3};

// #[cfg(feature = "python")]
// use numpy::{AllowTypeChange, PyArray1, PyArray2, PyArrayLike1};
#[cfg(feature = "python")]
use pyo3::prelude::*;

use crate::{
    Context,
    entity_component::{
        ComponentStorage, ComponentsView, EntityId,
        component::sparse_array::{Iter, SparseArray},
    },
};

/// A Scene Graph works as a tree-like structure establishing parent-child relationships between scene elements,
/// creating logical groupings where transformations (position, rotation, scale) applied to parent nodes automatically
/// affect all their children - simplifying complex object manipulation and animation.
#[derive(Debug)]
#[cfg_attr(feature = "python", pyclass)]
pub struct SceneGraph {
    nodes: SparseArray<Node>,
    first_root: Option<EntityId>,
    root_count: usize,
}

impl SceneGraph {
    /// Constructs a new, empty scene graph.
    ///
    /// The scene graph will not allocate until nodes are added.
    pub fn new(_ctx: &Context) -> Self {
        Self {
            nodes: SparseArray::new(),
            first_root: None,
            root_count: 0,
        }
    }

    /// Create a node in the scene graph for the given entity.
    /// If `parent` is `None`, the new node will be a root node.
    /// Otherwise, the new node will be a child of `parent`.
    ///
    /// It entity did not have a scene graph node, `None` is returned. If the entity had
    /// one, the node is updated and the old value is returned. All children of the old
    /// node will be removed.
    ///
    /// ## Panics
    ///
    /// Panics if `parent` references an entity with no assicated node.
    pub fn add(&mut self, entity: EntityId, parent: impl Into<Option<EntityId>>) -> Option<Node> {
        self.add_with_transform(entity, parent, Vec3::ZERO, Quat::IDENTITY, Vec3::ONE)
    }

    /// Create a node in the scene graph for the given entity.
    /// If `parent` is `None`, the new node will be a root node.
    /// Otherwise, the new node will be a child of `parent`.
    ///
    /// It entity did not have a scene graph node, `None` is returned. If the entity had
    /// one, the node is updated and the old value is returned. All children of the old
    /// node will be removed.
    ///
    /// ## Panics
    ///
    /// Panics if `parent` references an entity with no assicated node.
    pub fn add_with_transform(
        &mut self,
        entity: EntityId,
        parent: impl Into<Option<EntityId>>,
        local_translation: Vec3,
        local_rotation: Quat,
        local_scale: Vec3,
    ) -> Option<Node> {
        let parent = parent.into();
        let previous = self.remove(entity);

        let (parent_tranform, next_sibling) = match parent {
            Some(id) => {
                let parent = &mut self.nodes[id];
                parent.children_count += 1;
                (parent.global_transformation, &mut parent.first_child)
            }
            None => {
                self.root_count += 1;
                (Mat4::IDENTITY, &mut self.first_root)
            }
        };

        let (previous_sibling, next_sibling) = match *next_sibling {
            Some(next_sibling) => {
                // parent already has another child -> patch double linked list
                let next = &mut self.nodes[next_sibling];
                let previous_sibling = next.previous_sibling;
                next.previous_sibling = entity;
                self.nodes[previous_sibling].next_sibling = entity;
                (previous_sibling, next_sibling)
            }
            None => {
                *next_sibling = Some(entity);
                (entity, entity)
            }
        };

        let global_transformation = parent_tranform
            * Mat4::from_scale_rotation_translation(local_scale, local_rotation, local_translation);

        let node = Node {
            parent,
            children_count: 0,
            first_child: None,
            previous_sibling,
            next_sibling,
            local_translation,
            local_rotation,
            local_scale,
            global_transformation,
        };
        self.nodes.add(entity, node);

        previous
    }

    /// Removes the node assicated with `entity` from the scene graph and returns it.
    /// Returns `None` if `entity` does not have any node.
    pub fn remove(&mut self, entity: EntityId) -> Option<Node> {
        let node = self.nodes.remove(entity);
        if let Some(node) = &node {
            let parent = node.parent.map(|id| &mut self.nodes[id]);

            if node.next_sibling == entity {
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

    /// Returns an iterator visiting all root nodes once. A root node is a node with no parent.
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
    /// Panics if `entity` is not a node.
    pub fn parents(&self, entity: EntityId) -> ParentsIter<'_> {
        ParentsIter {
            scene_graph: &self,
            next: self[entity].parent,
        }
    }

    /// Returns an iterator visiting all sibling nodes once. A sibling is a node with the same parent.
    ///
    /// ## Panics
    ///
    /// Panics if `entity` is not a node.
    pub fn siblings(&self, entity: EntityId) -> SiblingsIter<'_> {
        SiblingsIter {
            scene_graph: self,
            first: entity,
            next: self.nodes[entity].next_sibling,
        }
    }

    /// Returns an iterator visiting all direct children nodes once.
    ///
    /// ## Panics
    ///
    /// Panics if `entity` is not a node.
    pub fn direct_children(&self, entity: EntityId) -> DirectChildrenIter<'_> {
        let parent = &self[entity];
        let next = parent.first_child.unwrap_or(entity);
        DirectChildrenIter {
            scene_graph: self,
            next,
            remaining: parent.children_count,
        }
    }

    /// Returns an iterator visition all children nodes once. Deeper children will always be visited after all their parent nodes.
    ///
    /// ## Panics
    ///
    /// Panics if `entity` is not a node.
    pub fn all_children(&self, entity: EntityId) -> AllChildrenIter<'_> {
        let parent = &self[entity];
        let next = parent.first_child.unwrap_or(entity);
        let states = vec![DepthIterState {
            next,
            remaining: parent.children_count,
        }];
        AllChildrenIter {
            scene_graph: self,
            states,
        }
    }

    /// Sets the node's local translation (if not `None`), rotation (if not `None`) and scale (if not `None`).
    /// See [Node::local_transformation] for more informations.
    /// This function will fail the node contains an invalid parent or if any of the children
    /// (direct and indirect) is invalid.
    ///
    /// ## Panics
    ///
    /// Panics if `entity` is not a node.
    pub fn set_local_transformation(
        &mut self,
        entity: EntityId,
        translation: impl Into<Option<Vec3>>,
        rotation: impl Into<Option<Quat>>,
        scale: impl Into<Option<Vec3>>,
    ) {
        let parent_transform = match self[entity].parent {
            Some(parent) => self[parent].global_transformation,
            None => Mat4::IDENTITY,
        };

        let node = &mut self.nodes[entity];
        node.local_translation = translation.into().unwrap_or(node.local_translation);
        node.local_rotation = rotation.into().unwrap_or(node.local_rotation);
        node.local_scale = scale.into().unwrap_or(node.local_scale);
        node.global_transformation = parent_transform * node.local_transformation();

        // update children global_transformation
        if let Some(next) = node.first_child {
            let mut states = vec![UpdateTransformIterState {
                next,
                remaining: node.children_count,
                parent_transform,
            }];

            while let Some(state) = states.last_mut() {
                if state.remaining > 0 {
                    state.remaining -= 1;
                    let node = &mut self.nodes[state.next];
                    node.global_transformation =
                        state.parent_transform * node.local_transformation();
                    state.next = node.next_sibling;
                    if let Some(next) = node.first_child {
                        states.push(UpdateTransformIterState {
                            next,
                            remaining: node.children_count,
                            parent_transform: node.global_transformation,
                        });
                    }
                } else {
                    states.pop();
                }
            }
        }
    }
}

impl ComponentsView<Node> for SceneGraph {
    type Iter<'a>
        = Iter<'a, Node>
    where
        Self: 'a,
        Node: 'a;

    fn has(&self, entity: EntityId) -> bool {
        self.nodes.has(entity)
    }

    fn get(&self, entity: EntityId) -> Option<&Node> {
        self.nodes.get(entity)
    }

    fn iter<'a>(&'a self) -> Self::Iter<'a> {
        self.nodes.iter()
    }
}

impl Index<EntityId> for SceneGraph {
    type Output = Node;

    fn index(&self, index: EntityId) -> &Self::Output {
        &self.nodes[index]
    }
}

// #[cfg(feature = "python")]
// #[pymethods]
// impl SceneGraph {
//     fn nodes(slf: &Bound<'_, Self>) -> Vec<PyNode> {
//         slf.borrow()
//             .nodes
//             .iter()
//             .map(|(&id, _)| PyNode {
//                 id,
//                 scene_graph: slf.clone().into(),
//             })
//             .collect()
//     }

//     /// Returns `true` if the scene graph contains the specified node.
//     pub fn has(&self, node: EntityId) -> bool {
//         self.has(node)
//     }
// }

/// A Scene Graph's node. See [SceneGraph] for more informations.
#[derive(Debug)]
pub struct Node {
    parent: Option<EntityId>,
    children_count: usize,
    first_child: Option<EntityId>,
    /// the previous sibling in the list of children for the parent
    previous_sibling: EntityId,
    /// the next sibling in the list of children for the parent
    next_sibling: EntityId,

    local_translation: Vec3,
    local_rotation: Quat,
    local_scale: Vec3,

    global_transformation: Mat4,
}

impl Node {
    /// Returns the parent's node id or `None` if the node is a root node.
    pub fn parent(&self) -> Option<EntityId> {
        self.parent
    }

    /// Returns the local translation of the node.
    /// See [`local_transformation`][Self::local_transformation] for more informations.
    pub fn local_translation(&self) -> Vec3 {
        self.local_translation
    }

    /// Returns the local rotation of the node.
    /// See [`local_transformation`][Self::local_transformation] for more informations.
    pub fn local_rotation(&self) -> Quat {
        self.local_rotation
    }

    /// Returns the local scale of the node.
    /// See [`local_transformation`][Self::local_transformation] for more informations.
    pub fn local_scale(&self) -> Vec3 {
        self.local_scale
    }

    /// Returns the local transformation of the node. The returns matrix can be used transform points
    /// from local space to the parent node's space. The local transformation is made up of
    /// the [local translation (`T`)][Self::local_translation], the [local rotation (`R`)][Self::local_rotation]
    /// and the [local scale (`S`)][Self::local_scale] in the `T * R * S` order (first the scale is applied to the point,
    /// then the rotation, and then the translation).
    /// When the node has no parent, the local transformation is identical to the global transformation.
    pub fn local_transformation(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            self.local_scale,
            self.local_rotation,
            self.local_translation,
        )
    }

    /// Returns the global transformation of the node. The returns matrix can be used transform points
    /// from local space to the global space. The global transformation of a node is the product of the
    /// global transformation matrix of its parent and its own [local transformation matrix][Self::local_transformation].
    /// When the node has no parent, the global transformation is identical to the local transformation.
    pub fn global_transformation(&self) -> Mat4 {
        self.global_transformation
    }
}

/// An iterator visiting all parent nodes in bottom up order. The last elements will always be a root node.
pub struct ParentsIter<'a> {
    scene_graph: &'a SceneGraph,
    next: Option<EntityId>,
}

impl<'a> Iterator for ParentsIter<'a> {
    type Item = (EntityId, &'a Node);

    fn next(&mut self) -> Option<Self::Item> {
        self.next.map(|id| {
            let node = &self.scene_graph[id];
            self.next = node.parent;
            (id, node)
        })
    }
}

/// An iterator visiting all root nodes once. A root node is a node with no parent.
pub struct RootsIter<'a> {
    scene_graph: &'a SceneGraph,
    next: Option<EntityId>,
    remaining: usize,
}

impl<'a> Iterator for RootsIter<'a> {
    type Item = (EntityId, &'a Node);

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining > 0 {
            self.remaining -= 1;
            let id = self.next.unwrap();
            let node = &self.scene_graph[id];
            let item = (id, node);
            self.next = Some(node.next_sibling);
            Some(item)
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

/// An iterator visiting all sibling nodes once. A sibling is a node with the same parent.
pub struct SiblingsIter<'a> {
    scene_graph: &'a SceneGraph,
    first: EntityId,
    next: EntityId,
}

impl<'a> Iterator for SiblingsIter<'a> {
    type Item = (EntityId, &'a Node);

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

/// An iterator visiting all direct children nodes once.
pub struct DirectChildrenIter<'a> {
    scene_graph: &'a SceneGraph,
    next: EntityId,
    remaining: usize,
}

impl<'a> Iterator for DirectChildrenIter<'a> {
    type Item = (EntityId, &'a Node);

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

/// An iterator visition all children nodes once. Deeper children will always be visited after all their parent nodes.
pub struct AllChildrenIter<'a> {
    scene_graph: &'a SceneGraph,
    states: Vec<DepthIterState>,
}

struct DepthIterState {
    next: EntityId,
    remaining: usize,
}

impl<'a> Iterator for AllChildrenIter<'a> {
    type Item = (EntityId, &'a Node);

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

struct DeleteIterState {
    next: EntityId,
    remaining: usize,
}

struct UpdateTransformIterState {
    next: EntityId,
    remaining: usize,
    parent_transform: Mat4,
}

// #[cfg(feature = "python")]
// #[pyclass(frozen)]
// pub struct PyNode {
//     id: EntityId,
//     scene_graph: Py<SceneGraph>,
// }

// #[cfg(feature = "python")]
// impl PyNode {
//     pub fn new(id: EntityId, scene_graph: Py<SceneGraph>) -> Self {
//         Self { id, scene_graph }
//     }

//     fn deleted_error(id: EntityId) -> PyErr {
//         pyo3::exceptions::PyRuntimeError::new_err(format!(
//             "Node {id} has been deleted from the scene graph"
//         ))
//     }

//     fn get<'a>(&self, scene_graph: &'a SceneGraph) -> PyResult<&'a Node> {
//         scene_graph
//             .get(self.id)
//             .ok_or_else(|| Self::deleted_error(self.id))
//     }

//     fn get_mut<'a>(&self, scene_graph: &'a mut SceneGraph) -> PyResult<&'a mut Node> {
//         scene_graph
//             .get_mut(self.id)
//             .ok_or_else(|| Self::deleted_error(self.id))
//     }
// }

// #[cfg(feature = "python")]
// #[pymethods]
// impl PyNode {
//     #[getter]
//     pub fn id(&self) -> EntityId {
//         self.id
//     }

//     #[getter]
//     fn name(&self, py: Python) -> PyResult<String> {
//         Ok(self.get(&self.scene_graph.borrow(py))?.name.clone())
//     }

//     #[setter]
//     fn set_name(&self, py: Python, name: String) -> PyResult<()> {
//         self.get_mut(&mut self.scene_graph.borrow_mut(py))?.name = name;
//         Ok(())
//     }

//     fn parent(&self, py: Python) -> PyResult<Option<PyNode>> {
//         Ok(self
//             .get(&self.scene_graph.borrow(py))?
//             .parent
//             .map(|id| PyNode {
//                 id,
//                 scene_graph: self.scene_graph.clone_ref(py),
//             }))
//     }

//     #[getter]
//     fn local_translation<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray1<f32>>> {
//         let translation = self.get(&self.scene_graph.borrow(py))?.local_translation;
//         Ok(PyArray1::from_slice(py, &translation.to_array()))
//     }

//     #[setter]
//     fn set_local_translation<'py>(
//         &self,
//         py: Python<'py>,
//         translation: PyArrayLike1<'py, f32, AllowTypeChange>,
//     ) -> PyResult<()> {
//         use glam::vec3;

//         let translation = translation.as_array();
//         if translation.dim() != 3 {
//             return Err(pyo3::exceptions::PyValueError::new_err(
//                 "translation.shape must be (3,)",
//             ));
//         }
//         self.scene_graph
//             .borrow_mut(py)
//             .set_local_transformation(
//                 self.id,
//                 vec3(translation[0], translation[1], translation[2]),
//                 None,
//                 None,
//             )
//             .map_err(|_| Self::deleted_error(self.id))
//     }

//     #[getter]
//     fn local_rotation<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray1<f32>>> {
//         let rotation = self.get(&self.scene_graph.borrow(py))?.local_rotation;
//         Ok(PyArray1::from_slice(
//             py,
//             &[rotation.w, rotation.x, rotation.y, rotation.z],
//         ))
//     }

//     #[setter]
//     fn set_local_rotation<'py>(
//         &self,
//         py: Python<'py>,
//         rotation: PyArrayLike1<'py, f32, AllowTypeChange>,
//     ) -> PyResult<()> {
//         use glam::quat;

//         let rotation = rotation.as_array();
//         if rotation.dim() != 4 {
//             return Err(pyo3::exceptions::PyValueError::new_err(
//                 "rotation.shape must be (4,)",
//             ));
//         }
//         self.scene_graph
//             .borrow_mut(py)
//             .set_local_transformation(
//                 self.id,
//                 None,
//                 quat(rotation[1], rotation[2], rotation[3], rotation[0]),
//                 None,
//             )
//             .map_err(|_| Self::deleted_error(self.id))
//     }

//     #[getter]
//     fn local_scale<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray1<f32>>> {
//         let scale = self.get(&self.scene_graph.borrow(py))?.local_scale;
//         Ok(PyArray1::from_slice(py, &scale.to_array()))
//     }

//     #[setter]
//     fn set_local_scale<'py>(
//         &self,
//         py: Python<'py>,
//         scale: PyArrayLike1<'py, f32, AllowTypeChange>,
//     ) -> PyResult<()> {
//         use glam::vec3;

//         let scale = scale.as_array();
//         if scale.dim() != 3 {
//             return Err(pyo3::exceptions::PyValueError::new_err(
//                 "scale.shape must be (3,)",
//             ));
//         }
//         self.scene_graph
//             .borrow_mut(py)
//             .set_local_transformation(self.id, None, None, vec3(scale[0], scale[1], scale[2]))
//             .map_err(|_| Self::deleted_error(self.id))
//     }

//     #[getter]
//     fn local_transformation<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray2<f32>>> {
//         use numpy::ndarray::aview2;

//         let transformation = self
//             .get(&self.scene_graph.borrow(py))?
//             .local_transformation()
//             .transpose()
//             .to_cols_array_2d();
//         let array = aview2(&transformation);
//         Ok(PyArray2::from_array(py, &array))
//     }

//     #[getter]
//     fn global_transformation<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray2<f32>>> {
//         use numpy::ndarray::aview2;

//         let transformation = self
//             .get(&self.scene_graph.borrow(py))?
//             .global_transformation()
//             .transpose()
//             .to_cols_array_2d();
//         let array = aview2(&transformation);
//         Ok(PyArray2::from_array(py, &array))
//     }
// }

// /// A Scene Graph node builder. See [SceneGraph] for more informations.
// #[must_use]
// #[derive(Debug, Clone)]
// pub struct NodeBuilder {
//     entity: EntityId,
//     parent: Option<EntityId>,
//     local_translation: Vec3,
//     local_rotation: Quat,
//     local_scale: Vec3,
// }

// impl NodeBuilder {
//     /// Creates a node for the given entity.
//     pub fn new(entity: EntityId) -> NodeBuilder {
//         NodeBuilder {
//             entity,
//             parent: None,
//             local_translation: Vec3::ZERO,
//             local_rotation: Quat::IDENTITY,
//             local_scale: Vec3::ONE,
//         }
//     }

//     // /// Sets the node Universally Unique Identifier (UUID). One is automatically generated if not provided.
//     // pub fn uuid(mut self, uuid: impl Into<NonNilUuid>) -> Self {
//     //     self.uuid = Some(uuid.into());
//     //     self
//     // }

//     // /// Sets the node name. Defaults to an empty string.
//     // pub fn name(mut self, name: impl Into<String>) -> Self {
//     //     self.name = name.into();
//     //     self
//     // }

//     // /// Sets the node parent. A node without any parent will be added as a root node.
//     // pub fn parent(mut self, parent: impl Into<EntityId>) -> Self {
//     //     self.parent = Some(parent.into());
//     //     self
//     // }

//     // /// Sets the translation component of the node local transform. Defaults to [`Vec3::ZERO`].
//     // /// See [`local_transformation`][Node::local_transformation] for more informations.
//     // pub fn local_translation(mut self, translation: impl Into<Vec3>) -> Self {
//     //     self.local_translation = Some(translation.into());
//     //     self
//     // }

//     // /// Sets the rotation component of the node local transform. Defaults to [`Quat::IDENTITY`].
//     // /// See [`local_transformation`][Node::local_transformation] for more informations.
//     // pub fn local_rotation(mut self, rotation: impl Into<Quat>) -> Self {
//     //     self.local_rotation = Some(rotation.into());
//     //     self
//     // }

//     // /// Sets the scale component of the node local transform. Defaults to [`Vec3::ONE`].
//     // /// See [`local_transformation`][Node::local_transformation] for more informations.
//     // pub fn local_scale(mut self, scale: impl Into<Vec3>) -> Self {
//     //     self.local_scale = Some(scale.into());
//     //     self
//     // }

//     // /// Builds the node and adds it to the scene graph.
//     // pub fn build(self, scene_graph: &mut SceneGraph) -> Result<EntityId, NodeBuilderError> {
//     //     let uuid = self
//     //         .uuid
//     //         .unwrap_or_else(|| NonNilUuid::new(Uuid::new_v4()).unwrap());
//     //     let id = EntityId { uuid };

//     //     let local_scale = self.local_scale.unwrap_or(Vec3::ONE);
//     //     let local_rotation = self.local_rotation.unwrap_or(Quat::IDENTITY);
//     //     let local_translation = self.local_translation.unwrap_or(Vec3::ZERO);

//     //     let local_transformation =
//     //         Mat4::from_scale_rotation_translation(local_scale, local_rotation, local_translation);

//     //     let global_transformation = match self.parent {
//     //         Some(parent) => {
//     //             let parent_data = scene_graph
//     //                 .nodes
//     //                 .get_mut(&parent)
//     //                 .ok_or(NodeBuilderError::ParentNodeNotFound(parent))?;
//     //             parent_data.children.push(id);
//     //             parent_data.global_transformation * local_transformation
//     //         }
//     //         None => {
//     //             scene_graph.root_nodes.push(id);
//     //             local_transformation
//     //         }
//     //     };

//     //     let node = Node {
//     //         name: self.name,
//     //         parent: self.parent,
//     //         children: Vec::new(),
//     //         local_translation,
//     //         local_rotation,
//     //         local_scale,
//     //         global_transformation,
//     //     };

//     //     match scene_graph.nodes.insert(id, node) {
//     //         None => Ok(id),
//     //         Some(node) => {
//     //             scene_graph.nodes.insert(id, node);
//     //             Err(NodeBuilderError::UuidNotUnique(id.uuid))
//     //         }
//     //     }
//     // }
// }

// /// Error when [`NodeBuilder::build()`] fails.
// #[derive(Debug, Error)]
// pub enum NodeBuilderError {
//     #[error("the scene graph already contains a node with UUID {0}")]
//     UuidNotUnique(NonNilUuid),
//     #[error("no node found for {0}")]
//     ParentNodeNotFound(EntityId),
// }

// #[derive(Debug, Error)]
// #[error("no node found for {0}")]
// #[cfg_attr(feature = "python", pyclass)]
// pub struct NodeNotFoundError(pub EntityId);

#[cfg(test)]
mod tests {
    use crate::entity_component::EntityManager;

    use super::*;

    #[test]
    fn test_scene_graph_roots() {
        let ctx = pollster::block_on(Context::new());
        let mut entity_manager = EntityManager::new();
        let mut scene_graph = SceneGraph::new(&ctx);

        let mut roots = scene_graph.roots();
        assert_eq!(0, roots.len());
        assert!(roots.next().is_none());

        let root_0 = entity_manager.new_entity();

        let previous = scene_graph.add(root_0, None);
        assert!(previous.is_none());
        assert_eq!(1, scene_graph.root_count);
        assert_eq!(Some(root_0), scene_graph.first_root);
        assert_eq!(root_0, scene_graph[root_0].previous_sibling);
        assert_eq!(root_0, scene_graph[root_0].next_sibling);

        let root_1 = entity_manager.new_entity();

        let previous = scene_graph.add(root_1, None);
        assert!(previous.is_none());
        assert_eq!(2, scene_graph.root_count);
        assert_eq!(Some(root_0), scene_graph.first_root);
        assert_eq!(root_1, scene_graph[root_0].previous_sibling);
        assert_eq!(root_0, scene_graph[root_1].previous_sibling);
        assert_eq!(root_1, scene_graph[root_0].next_sibling);
        assert_eq!(root_0, scene_graph[root_1].next_sibling);

        let root_2 = entity_manager.new_entity();

        let previous = scene_graph.add(root_2, None);
        assert!(previous.is_none());
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
}
