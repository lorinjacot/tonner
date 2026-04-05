use std::{
    any::{Any, type_name},
    collections::HashMap,
    fmt::Debug,
    ops::Deref,
    sync::{Arc, Mutex},
};

#[cfg(feature = "python")]
use pyo3::prelude::*;
use uuid::Uuid;

/// The type used to uniquely identified each [World] field.
///
/// This field should stay unchanged between compilations and across different plaforms/OS. This is especially important for (des)serialization.
/// (Des)serialization is required to save, load and sync [World]s.
pub type FieldId = Uuid;

type Fields = HashMap<FieldId, Arc<dyn DynamicField>>;

/// A world is a collection of key-value pairs.
#[derive(Debug)]
pub struct World {
    fields: Mutex<Fields>,
}

impl World {
    /// Creates an empty `World`.
    pub fn new() -> World {
        World {
            fields: Mutex::new(HashMap::new()),
        }
    }

    /// Adds a new field to the world.
    ///
    /// If the world did not have this field present, `None` is returned. If the world did have this field present, the value is updated,
    /// and the old value is returned.
    ///
    /// ## Panics
    ///
    /// This function will panic if the world already contains a field with the same id but of a different type.
    pub fn add<Field: StaticField>(&self, field: Arc<Field>) -> Option<Arc<Field>> {
        self.add_dynamic(field)
    }

    /// Adds a new field to the world.
    ///
    /// If the world did not have this field present, `None` is returned. If the world did have this field present, the value is updated,
    /// and the old value is returned.
    ///
    /// ## Panics
    ///
    /// This function will panic if the world already contains a field with the same id but of a different type.
    pub fn add_dynamic<Field: DynamicField>(&self, field: Arc<Field>) -> Option<Arc<Field>> {
        self.add_any(field).map(|old| {
            let any = old as Arc<dyn Any>;
            dbg!(any.deref().type_id());
            Arc::clone(
                any.downcast_ref()
                    .expect("the type of a world field should never change"),
            )
        })
    }

    /// Adds a new field to the world. If possible, it is preferable to use [`World::insert`]. If possible, it is preferable to use [`World::add_dynamic`].
    ///
    /// If the world did not have this field present, `None` is returned. If the world did have this field present, the value is updated,
    /// and the old value is returned.
    ///
    /// This method should be used with great care: each field (and therefore [FieldId]) should always have the same underlying concrete type.
    /// This is especially important when mixing [World::add], [World::add_dynamic] and [World::add_any].
    fn add_any(&self, field: Arc<dyn DynamicField>) -> Option<Arc<dyn DynamicField>> {
        self.fields.lock().unwrap().insert(field.id(), field)
    }

    /// Returns `true` if and only the world contains the `StaticField`.
    pub fn contains<Field: StaticField>(&self) -> bool {
        self.contains_dynamic(Field::ID)
    }

    /// Returns `true` if and only the world contains a field for `id`.
    pub fn contains_dynamic(&self, id: FieldId) -> bool {
        self.fields.lock().unwrap().contains_key(&id)
    }

    /// Returns a shared pointer to the world field. Returns `None` if the world does not contain the field.
    ///
    /// ## Panics
    ///
    /// This function will panic if the world contains a field with the same id but of a different type.
    pub fn get<Field: StaticField>(&self) -> Option<Arc<Field>> {
        self.get_dynamic(Field::ID)
    }

    /// Returns a shared pointer to the world field. Returns `None` if the world does not contain the field.
    ///
    /// ## Panics
    ///
    /// This function will panic if the world contains a field with the same id but of a different type.
    pub fn get_dynamic<Field: DynamicField>(&self, id: FieldId) -> Option<Arc<Field>> {
        let any = self.get_any(id)? as Arc<dyn Any>;
        Some(Arc::clone(
            any.downcast_ref()
                .expect("the type of a world field should never change"),
        ))
    }

    /// Returns a shared pointer to the world field or `None` if the world does not contain the field. If possible, it is preferable to use [`World::get_dynamic`].
    ///
    /// This method should be used with great care: each field (and therefore [FieldId]) should always have the same underlying concrete type.
    /// This is especially important when mixing [World::get], [World::get_dynamic] and [World::get_any].
    fn get_any(&self, id: FieldId) -> Option<Arc<dyn DynamicField>> {
        self.fields.lock().unwrap().get(&id).cloned()
    }

    /// Removes a field from the world, returning the value of the field if the field was previously in the world.
    pub fn remove<Field: StaticField>(&self) -> Option<Arc<Field>> {
        self.remove_dynamic(Field::ID)
    }

    /// Removes a field from the world, returning the value of the field if the field was previously in the world.
    pub fn remove_dynamic<Field: DynamicField>(&self, id: FieldId) -> Option<Arc<Field>> {
        self.remove_any(id).map(|old| {
            let any = old as Arc<dyn Any>;
            Arc::clone(
                any.downcast_ref()
                    .expect("the type of a world field should never change"),
            )
        })
    }

    /// Removes a field from the world, returning the value of the field if the field was previously in the world. If possible, it is preferable to use [`World::remove_dynamic`].
    ///
    /// This method should be used with great care: each field (and therefore [FieldId]) should always have the same underlying concrete type.
    /// This is especially important when mixing [World::get], [World::remove_dynamic] and [World::remove_any].
    fn remove_any(&self, id: FieldId) -> Option<Arc<dyn DynamicField>> {
        self.fields.lock().unwrap().remove(&id)
    }
}

#[cfg(feature = "python")]
#[pyclass(name = "World", frozen)]
pub struct PyWorld(pub Arc<World>);

#[cfg(feature = "python")]
#[pymethods]
impl PyWorld {
    #[new]
    pub fn new() -> PyWorld {
        PyWorld(Arc::new(World::new()))
    }

    // fn new_entity(&self) -> Entity {
    //     let id = self.0.entity_manager.lock().unwrap().new_entity();
    //     Entity::new(id, self.0.clone())
    // }
}

/// A [World] field whose [FieldId] is known at compile time. This means the world can have
/// one field per `StaticField` type.
pub trait StaticField: Debug + Send + Sync + 'static {
    /// Field unique id.
    ///
    /// This value should stay unchanged between compilations and across different plaforms/OS. This is especially important for (des)serialization.
    /// (Des)serialization is required to save, load and sync [World]s.
    const ID: FieldId;

    /// Turns the field into a python object. Returns `None` if the field is not available in python.
    ///
    /// If the `StaticField` already implements [`pyo3::PyClass`], this method may just wrap it into a `Bound` before
    /// calling [`Bound::as_any`]. If the `StaticField` does not implement [`pyo3::PyClass`] but should still be accessible
    /// from python, this method may return a wrapper created using `#[pyclass]`. The default implementation returns `None`.
    #[cfg(feature = "python")]
    #[allow(unused_variables)]
    fn as_py<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyAny>> {
        None
    }
}

/// A [World] field whose [FieldId] is known at runtime. This means the world can have multiple field
/// per `DynamicField` type. This can be useful for field comming from dynamically-typed languaged like
/// python.
///
/// All [`StaticField`] are also `DynamicField`.
pub trait DynamicField: Debug + Any + Send + Sync {
    /// Field unique id. This value should stay unchanged throughout the lifetime of the app, between compilations and across different plaforms/OS.
    /// It is a logic error to change a field id. This is especially important for correct field access and (des)serialization.
    fn id(&self) -> FieldId;

    /// Turns the field into a python object. Returns `None` if the field is not available in python.
    ///
    /// If the `DynamicField` already implements [`pyo3::PyClass`], this method may just wrap it into a `Bound` before
    /// calling [`Bound::as_any`]. If the `DynamicField` does not implement [`pyo3::PyClass`] but should still be accessible
    /// from python, this method may return a wrapper created using `#[pyclass]`. The default implementation returns `None`.
    #[cfg(feature = "python")]
    #[allow(unused_variables)]
    fn as_py<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyAny>> {
        None
    }
}

impl<T: StaticField> DynamicField for T {
    fn id(&self) -> FieldId {
        Self::ID
    }

    fn as_py<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyAny>> {
        StaticField::as_py(self, py)
    }
}
