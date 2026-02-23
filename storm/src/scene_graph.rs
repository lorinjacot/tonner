use std::{
    collections::{HashMap, hash_map},
    fmt::Display,
    iter::FusedIterator,
    ops::{Index, IndexMut},
};

use glam::{Mat4, Quat, Vec3};
use thiserror::Error;
use uuid::{NonNilUuid, Uuid};

#[cfg(feature = "pyo3")]
use numpy::{AllowTypeChange, PyArray1, PyArray2, PyArrayLike1};
#[cfg(feature = "pyo3")]
use pyo3::prelude::*;

use crate::Context;

/// A Scene Graph works as a tree-like structure establishing parent-child relationships between scene elements,
/// creating logical groupings where transformations (position, rotation, scale) applied to parent nodes automatically
/// affect all their children - simplifying complex object manipulation and animation.
#[derive(Debug)]
#[cfg_attr(feature = "pyo3", pyclass)]
pub struct SceneGraph {
    nodes: HashMap<NodeId, Node>,
    root_nodes: Vec<NodeId>,
}

impl SceneGraph {
    pub fn new(_ctx: &Context) -> Self {
        Self {
            nodes: HashMap::new(),
            root_nodes: Vec::new(),
        }
    }

    /// Returns a reference to the node corresponding to the id.
    pub fn get(&self, node: NodeId) -> Option<&Node> {
        self.nodes.get(&node)
    }

    /// Returns a mutable reference to the node corresponding to the id.
    pub fn get_mut(&mut self, node: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(&node)
    }

    /// An iterator visiting all nodes contained in the Scene Graph.
    pub fn iter(&self) -> NodeIter<'_> {
        NodeIter {
            base: self.nodes.iter(),
        }
    }

    /// An iterator visiting all nodes contained in the Scene Graph in arbitrary order, with mutable references to the values.
    pub fn iter_mut(&mut self) -> NodeMutIter<'_> {
        NodeMutIter {
            base: self.nodes.iter_mut(),
        }
    }

    fn recursively_update_global_transformation(
        &mut self,
        node: NodeId,
        parent_global_transformation: Mat4,
    ) -> Result<(), NodeNotFoundError> {
        let node = self.get_mut(node).ok_or(NodeNotFoundError(node))?;
        let global_transformation = parent_global_transformation * node.local_transformation();
        node.global_transformation = global_transformation;
        for child in node.children.clone() {
            self.recursively_update_global_transformation(child, global_transformation)?;
        }
        Ok(())
    }

    /// Sets the node's local translation (if not `None`), rotation (if not `None`) and scale (if not `None`).
    /// See [Node::local_transformation] for more informations.
    /// This function will fail the node contains an invalid parent or if any of the children
    /// (direct and indirect) is invalid.
    pub fn set_local_transformation(
        &mut self,
        node: NodeId,
        translation: impl Into<Option<Vec3>>,
        rotation: impl Into<Option<Quat>>,
        scale: impl Into<Option<Vec3>>,
    ) -> Result<(), NodeNotFoundError> {
        let parent_transformation = {
            let node = self.get_mut(node).ok_or(NodeNotFoundError(node))?;
            node.local_translation = translation.into().unwrap_or(node.local_translation);
            node.local_rotation = rotation.into().unwrap_or(node.local_rotation);
            node.local_scale = scale.into().unwrap_or(node.local_scale);
            match node.parent {
                Some(parent) => {
                    self.get(parent)
                        .ok_or(NodeNotFoundError(parent))?
                        .global_transformation
                }
                None => Mat4::IDENTITY,
            }
        };
        self.recursively_update_global_transformation(node, parent_transformation)
    }
}

#[cfg(feature = "pyo3")]
#[pymethods]
impl SceneGraph {
    fn nodes(slf: &Bound<'_, Self>) -> Vec<PyNode> {
        slf.borrow()
            .nodes
            .iter()
            .map(|(&id, _)| PyNode {
                id,
                scene_graph: slf.clone().into(),
            })
            .collect()
    }

    /// Returns `true` if the scene graph contains the specified node.
    pub fn contains(&self, node: NodeId) -> bool {
        self.nodes.contains_key(&node)
    }
}

impl Index<NodeId> for SceneGraph {
    type Output = Node;

    fn index(&self, index: NodeId) -> &Self::Output {
        self.get(index).expect("no node found for node id")
    }
}

impl IndexMut<NodeId> for SceneGraph {
    fn index_mut(&mut self, index: NodeId) -> &mut Self::Output {
        self.get_mut(index).expect("no node found for node id")
    }
}

impl<'a> IntoIterator for &'a SceneGraph {
    type Item = (NodeId, &'a Node);
    type IntoIter = NodeIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut SceneGraph {
    type Item = (NodeId, &'a mut Node);
    type IntoIter = NodeMutIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

/// An iterator over all nodes contained in a [`SceneGraph`].
///
/// This `struct` is created by the
#[derive(Debug, Clone, Default)]
pub struct NodeIter<'a> {
    base: hash_map::Iter<'a, NodeId, Node>,
}

impl<'a> Iterator for NodeIter<'a> {
    type Item = (NodeId, &'a Node);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.base.next().map(|(&id, node)| (id, node))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.base.size_hint()
    }

    #[inline]
    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.base.count()
    }

    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.base.fold(init, |b, (&id, node)| f(b, (id, node)))
    }
}

impl<'a> ExactSizeIterator for NodeIter<'a> {
    #[inline]
    fn len(&self) -> usize {
        self.base.len()
    }
}

impl<'a> FusedIterator for NodeIter<'a> {}

/// An mutable iterator over all nodes contained in a [`SceneGraph`].
///
/// This `struct` is created by the
#[derive(Debug, Default)]
pub struct NodeMutIter<'a> {
    base: hash_map::IterMut<'a, NodeId, Node>,
}

impl<'a> Iterator for NodeMutIter<'a> {
    type Item = (NodeId, &'a mut Node);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.base.next().map(|(&id, node)| (id, node))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.base.size_hint()
    }

    #[inline]
    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.base.count()
    }

    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.base.fold(init, |b, (&id, node)| f(b, (id, node)))
    }
}

impl<'a> ExactSizeIterator for NodeMutIter<'a> {
    #[inline]
    fn len(&self) -> usize {
        self.base.len()
    }
}

impl<'a> FusedIterator for NodeMutIter<'a> {}

/// A unique id for a Scene Graph node. Each node has one and only one id. The id for a given node will never change.
///
/// Node that `Option<NodeId>` takes up the same space as `NodeId`.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "pyo3", pyclass(frozen, str, from_py_object))]
pub struct NodeId {
    uuid: NonNilUuid,
}

impl Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NodeId({})", self.uuid)
    }
}

/// A Scene Graph's node. See [SceneGraph] for more informations.
#[derive(Debug)]
pub struct Node {
    /// Name of the node. Does not need to be unique. Can be used for debugging and displaying.
    pub name: String,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    local_translation: Vec3,
    local_rotation: Quat,
    local_scale: Vec3,
    global_transformation: Mat4,
}

impl Node {
    /// Returns the parent's node id or `None` if the node is a root node.
    pub fn parent(&self) -> Option<NodeId> {
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

#[cfg(feature = "pyo3")]
#[pyclass(frozen)]
struct PyNode {
    id: NodeId,
    scene_graph: Py<SceneGraph>,
}

#[cfg(feature = "pyo3")]
impl PyNode {
    fn deleted_error(id: NodeId) -> PyErr {
        pyo3::exceptions::PyRuntimeError::new_err(format!(
            "Node {id} has been deleted from the scene graph"
        ))
    }

    fn get<'a>(&self, scene_graph: &'a SceneGraph) -> PyResult<&'a Node> {
        scene_graph
            .get(self.id)
            .ok_or_else(|| Self::deleted_error(self.id))
    }

    fn get_mut<'a>(&self, scene_graph: &'a mut SceneGraph) -> PyResult<&'a mut Node> {
        scene_graph
            .get_mut(self.id)
            .ok_or_else(|| Self::deleted_error(self.id))
    }
}

#[cfg(feature = "pyo3")]
#[pymethods]
impl PyNode {
    #[getter]
    fn id(&self) -> NodeId {
        self.id
    }

    #[getter]
    fn name(&self, py: Python) -> PyResult<String> {
        Ok(self.get(&self.scene_graph.borrow(py))?.name.clone())
    }

    #[setter]
    fn set_name(&self, py: Python, name: String) -> PyResult<()> {
        self.get_mut(&mut self.scene_graph.borrow_mut(py))?.name = name;
        Ok(())
    }

    fn parent(&self, py: Python) -> PyResult<Option<PyNode>> {
        Ok(self
            .get(&self.scene_graph.borrow(py))?
            .parent
            .map(|id| PyNode {
                id,
                scene_graph: self.scene_graph.clone_ref(py),
            }))
    }

    #[getter]
    fn local_translation<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray1<f32>>> {
        let translation = self.get(&self.scene_graph.borrow(py))?.local_translation;
        Ok(PyArray1::from_slice(py, &translation.to_array()))
    }

    #[setter]
    fn set_local_translation<'py>(
        &self,
        py: Python<'py>,
        translation: PyArrayLike1<'py, f32, AllowTypeChange>,
    ) -> PyResult<()> {
        use glam::vec3;

        let translation = translation.as_array();
        if translation.dim() != 3 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "translation.shape must be (3,)",
            ));
        }
        self.scene_graph
            .borrow_mut(py)
            .set_local_transformation(
                self.id,
                vec3(translation[0], translation[1], translation[2]),
                None,
                None,
            )
            .map_err(|_| Self::deleted_error(self.id))
    }

    #[getter]
    fn local_rotation<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray1<f32>>> {
        let rotation = self.get(&self.scene_graph.borrow(py))?.local_rotation;
        Ok(PyArray1::from_slice(py, &rotation.to_array()))
    }

    #[setter]
    fn set_local_rotation<'py>(
        &self,
        py: Python<'py>,
        rotation: PyArrayLike1<'py, f32, AllowTypeChange>,
    ) -> PyResult<()> {
        use glam::quat;

        let rotation = rotation.as_array();
        if rotation.dim() != 3 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "rotation.shape must be (4,)",
            ));
        }
        self.scene_graph
            .borrow_mut(py)
            .set_local_transformation(
                self.id,
                None,
                quat(rotation[0], rotation[1], rotation[2], rotation[3]),
                None,
            )
            .map_err(|_| Self::deleted_error(self.id))
    }

    #[getter]
    fn local_scale<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray1<f32>>> {
        let scale = self.get(&self.scene_graph.borrow(py))?.local_scale;
        Ok(PyArray1::from_slice(py, &scale.to_array()))
    }

    #[setter]
    fn set_local_scale<'py>(
        &self,
        py: Python<'py>,
        scale: PyArrayLike1<'py, f32, AllowTypeChange>,
    ) -> PyResult<()> {
        use glam::vec3;

        let scale = scale.as_array();
        if scale.dim() != 3 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "scale.shape must be (3,)",
            ));
        }
        self.scene_graph
            .borrow_mut(py)
            .set_local_transformation(self.id, None, None, vec3(scale[0], scale[1], scale[2]))
            .map_err(|_| Self::deleted_error(self.id))
    }

    #[getter]
    fn local_transformation<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray2<f32>>> {
        use numpy::ndarray::aview2;

        let transformation = self
            .get(&self.scene_graph.borrow(py))?
            .local_transformation()
            .transpose()
            .to_cols_array_2d();
        let array = aview2(&transformation);
        Ok(PyArray2::from_array(py, &array))
    }

    #[getter]
    fn global_transformation<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray2<f32>>> {
        use numpy::ndarray::aview2;

        let transformation = self
            .get(&self.scene_graph.borrow(py))?
            .global_transformation()
            .transpose()
            .to_cols_array_2d();
        let array = aview2(&transformation);
        Ok(PyArray2::from_array(py, &array))
    }
}

/// A Scene Graph node builder. See [SceneGraph] for more informations.
#[must_use]
#[derive(Debug, Clone)]
pub struct NodeBuilder {
    uuid: Option<NonNilUuid>,
    name: String,
    parent: Option<NodeId>,
    local_translation: Option<Vec3>,
    local_rotation: Option<Quat>,
    local_scale: Option<Vec3>,
}

impl Default for NodeBuilder {
    fn default() -> Self {
        Self {
            uuid: None,
            name: String::new(),
            parent: None,
            local_translation: None,
            local_rotation: None,
            local_scale: None,
        }
    }
}

impl NodeBuilder {
    /// Sets the node Universally Unique Identifier (UUID). One is automatically generated if not provided.
    pub fn uuid(mut self, uuid: impl Into<NonNilUuid>) -> Self {
        self.uuid = Some(uuid.into());
        self
    }

    /// Sets the node name. Defaults to an empty string.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Sets the node parent. A node without any parent will be added as a root node.
    pub fn parent(mut self, parent: impl Into<NodeId>) -> Self {
        self.parent = Some(parent.into());
        self
    }

    /// Sets the translation component of the node local transform. Defaults to [`Vec3::ZERO`].
    /// See [`local_transformation`][Node::local_transformation] for more informations.
    pub fn local_translation(mut self, translation: impl Into<Vec3>) -> Self {
        self.local_translation = Some(translation.into());
        self
    }

    /// Sets the rotation component of the node local transform. Defaults to [`Quat::IDENTITY`].
    /// See [`local_transformation`][Node::local_transformation] for more informations.
    pub fn local_rotation(mut self, rotation: impl Into<Quat>) -> Self {
        self.local_rotation = Some(rotation.into());
        self
    }

    /// Sets the scale component of the node local transform. Defaults to [`Vec3::ONE`].
    /// See [`local_transformation`][Node::local_transformation] for more informations.
    pub fn local_scale(mut self, scale: impl Into<Vec3>) -> Self {
        self.local_scale = Some(scale.into());
        self
    }

    /// Builds the node and adds it to the scene graph.
    pub fn build(self, scene_graph: &mut SceneGraph) -> Result<NodeId, NodeBuilderError> {
        let uuid = self
            .uuid
            .unwrap_or_else(|| NonNilUuid::new(Uuid::new_v4()).unwrap());
        let id = NodeId { uuid };

        let local_scale = self.local_scale.unwrap_or(Vec3::ONE);
        let local_rotation = self.local_rotation.unwrap_or(Quat::IDENTITY);
        let local_translation = self.local_translation.unwrap_or(Vec3::ZERO);

        let local_transformation =
            Mat4::from_scale_rotation_translation(local_scale, local_rotation, local_translation);

        let global_transformation = match self.parent {
            Some(parent) => {
                let parent_data = scene_graph
                    .nodes
                    .get_mut(&parent)
                    .ok_or(NodeBuilderError::ParentNodeNotFound(parent))?;
                parent_data.children.push(id);
                parent_data.global_transformation * local_transformation
            }
            None => {
                scene_graph.root_nodes.push(id);
                local_transformation
            }
        };

        let node = Node {
            name: self.name,
            parent: self.parent,
            children: Vec::new(),
            local_translation,
            local_rotation,
            local_scale,
            global_transformation,
        };

        match scene_graph.nodes.insert(id, node) {
            None => Ok(id),
            Some(node) => {
                scene_graph.nodes.insert(id, node);
                Err(NodeBuilderError::UuidNotUnique(id.uuid))
            }
        }
    }
}

/// Error when [`NodeBuilder::build()`] fails.
#[derive(Debug, Error)]
pub enum NodeBuilderError {
    #[error("the scene graph already contains a node with UUID {0}")]
    UuidNotUnique(NonNilUuid),
    #[error("no node found for {0}")]
    ParentNodeNotFound(NodeId),
}

#[derive(Debug, Error)]
#[error("no node found for {0}")]
#[cfg_attr(feature = "pyo3", pyclass)]
pub struct NodeNotFoundError(pub NodeId);
