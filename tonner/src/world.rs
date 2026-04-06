use std::{
    any::Any,
    collections::HashMap,
    fmt::Debug,
    sync::{Arc, Mutex},
};

#[cfg(feature = "python")]
use pyo3::{prelude::*, types::PyType};
use uuid::Uuid;

#[cfg(feature = "python")]
use crate::Context;

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
        self.fields
            .lock()
            .unwrap()
            .insert(field.id(), field)
            .map(|old| {
                let any = old as Arc<dyn Any + Send + Sync>;
                any.downcast()
                    .expect("the type of a world field should never change")
            })
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
        self.get_any(id).map(|field| {
            let any = field as Arc<dyn Any + Send + Sync>;
            any.downcast()
                .expect("the type of a world field should never change")
        })
    }

    /// Returns a shared pointer to the world field or `None` if the world does not contain the field. If possible, it is preferable to use [`World::get_dynamic`].
    ///
    /// This method should be used with great care: each field (and therefore [FieldId]) should always have the same underlying concrete type.
    /// This is especially important when mixing [World::get], [World::get_dynamic] and [World::get_any].
    fn get_any(&self, id: FieldId) -> Option<Arc<dyn DynamicField>> {
        self.fields.lock().unwrap().get(&id).cloned()
    }

    /// Removes a field from the world, returning the value of the field if the field was previously in the world.
    ///
    /// ## Panics
    ///
    /// This function will panic if the world contains a field with the same id but of a different type.
    pub fn remove<Field: StaticField>(&self) -> Option<Arc<Field>> {
        self.remove_dynamic(Field::ID)
    }

    /// Removes a field from the world, returning the value of the field if the field was previously in the world.
    ///
    /// ## Panics
    ///
    /// This function will panic if the world contains a field with the same id but of a different type.
    pub fn remove_dynamic<Field: DynamicField>(&self, id: FieldId) -> Option<Arc<Field>> {
        self.fields.lock().unwrap().remove(&id).map(|old| {
            let any = old as Arc<dyn Any + Send + Sync>;
            any.downcast()
                .expect("the type of a world field should never change")
        })
    }
}

/// A world is a collection of key-value pairs.
#[cfg(feature = "python")]
#[pyclass(name = "World", frozen)]
pub struct PyWorld(pub Arc<World>);

#[cfg(feature = "python")]
#[pymethods]
impl PyWorld {
    /// Creates a new world.
    #[new]
    pub fn new(ctx: &Context) -> PyWorld {
        let world = World::new();
        world.add(Arc::new(ctx.clone()));
        PyWorld(Arc::new(world))
    }

    /// Returns the world field or `None` if the world does not have the field.
    pub fn get<'py>(
        &self,
        py: Python<'py>,
        field: Bound<'py, PyType>,
    ) -> PyResult<Option<Bound<'py, WorldField>>> {
        let id = field.call_method0("id")?;
        let id = id.extract()?;
        Ok(self.0.get_any(id).map(|field| field.as_py(py)).flatten())
    }
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
    fn as_py<'py>(&self, py: Python<'py>) -> Option<Bound<'py, WorldField>> {
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
    fn as_py<'py>(&self, py: Python<'py>) -> Option<Bound<'py, WorldField>> {
        None
    }
}

impl<T: StaticField> DynamicField for T {
    fn id(&self) -> FieldId {
        Self::ID
    }

    #[cfg(feature = "python")]
    fn as_py<'py>(&self, py: Python<'py>) -> Option<Bound<'py, WorldField>> {
        StaticField::as_py(self, py)
    }
}

/// Base class of all [World] fields.
#[cfg(feature = "python")]
#[pyclass(frozen, subclass)]
pub struct WorldField;
