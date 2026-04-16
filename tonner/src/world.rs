use std::{
    any::Any,
    collections::HashMap,
    fmt::Debug,
    sync::{Arc, Mutex},
};

#[cfg(feature = "python")]
use pyo3::prelude::*;
use uuid::Uuid;

use crate::{
    Context,
    entity_component::{EntityId, EntityManager},
    scene_graph::SceneGraph,
};

pub trait WorldRef {
    fn get<Field: StaticField>(&self) -> Option<Arc<Field>> {
        self.get_dynamic(Field::ID)
    }
    
    fn get_dynamic<Field: DynamicField>(&self, id: FieldId) -> Option<Arc<Field>>;
}

#[derive(Debug, Default)]
pub struct World {
    fields: HashMap<Uuid, Arc<dyn DynamicField>>,
}

impl WorldRef for World {
    fn get_dynamic<Field: DynamicField>(&self, id: FieldId) -> Option<Arc<Field>> {
        self.fields.get(&id).map(|field| {
            let any = field.clone() as Arc<dyn Any + Send + Sync>;
            any.downcast().expect("world field id should be unique")
        })
    }
}

pub type FieldId = Uuid;

pub trait StaticField: Send + Sync + Debug + 'static {
    const ID: FieldId;
}

pub trait DynamicField: Any + Send + Sync + Debug {
    fn id(&self) -> FieldId;
}

impl<T: StaticField> DynamicField for T {
    fn id(&self) -> Uuid {
        Self::ID
    }
}

/// An handle to a World. A world is a collection of entities.
///
/// A world can:
/// - be rendered
/// - have physics
/// - ...
///
/// This type is not needed if only the rust API is used.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "python", pyclass(frozen, skip_from_py_object))]
pub struct WorldHandle(Arc<Mutex<World>>);

impl WorldRef for WorldHandle {
    fn get_dynamic<Field: DynamicField>(&self, id: FieldId) -> Option<Arc<Field>> {
        self.0.lock().unwrap().get_dynamic(id)
    }
}

impl From<World> for WorldHandle {
    fn from(value: World) -> Self {
        WorldHandle(Arc::new(Mutex::new(value)))
    }
}

#[derive(Debug)]
pub struct TonnerWorld {
    context: Context,
    pub entity_manager: EntityManager,
    pub scene_graph: Arc<Mutex<SceneGraph>>,
}

impl TonnerWorld {
    pub fn new(context: Context) -> TonnerWorld {
        let scene_graph = SceneGraph::new(&context);
        TonnerWorld {
            context,
            entity_manager: EntityManager::new(),
            scene_graph: Arc::new(Mutex::new(scene_graph)),
        }
    }

    pub fn context(&self) -> &Context {
        &self.context
    }
}

/// An handle to a world. A world is a collection of entities.
///
/// A world can:
/// - be rendered
/// - have physics
/// - ...
///
/// This type is not needed if only the rust API is used.
#[derive(Debug, Clone)]
#[cfg_attr(
    feature = "python",
    pyclass(name = "World", dict, skip_from_py_object, frozen)
)]
pub struct TonnerWorldHandle {
    pub world: Arc<Mutex<TonnerWorld>>,
}

#[cfg(feature = "python")]
#[pymethods]
impl TonnerWorldHandle {
    #[new]
    fn py_new(ctx: &Context) -> TonnerWorldHandle {
        let world = TonnerWorld::new(ctx.clone());

        TonnerWorldHandle {
            world: Arc::new(Mutex::new(world)),
        }
    }

    fn new_entity(&self) -> TonnerEntityHandle {
        let entity = self.world.lock().unwrap().entity_manager.new_entity();
        TonnerEntityHandle {
            entity,
            world: self.clone(),
        }
    }
}

/// An handle to a World entity. An entity can be anything living inside a World.
///
/// An entity (from the Entity-Component-System (ECS) architecture) is a general-purpose
/// object identified by a unique ID. It acts as a container and query interface for
/// components — plain data objects that define the entity's characteristics and behaviour.
/// Systems then operate on entities that possess specific combinations of components.
///
/// This type is not needed if only the rust API is used.
#[derive(Debug, Clone)]
#[cfg_attr(
    feature = "python",
    pyclass(name = "TonnerEntity", dict, skip_from_py_object, frozen)
)]
pub struct TonnerEntityHandle {
    #[pyo3(get)]
    pub entity: EntityId,
    world: TonnerWorldHandle,
}

impl TonnerEntityHandle {
    pub fn world(&self) -> &TonnerWorldHandle {
        &self.world
    }
}

#[cfg(feature = "python")]
#[pymethods]
impl TonnerEntityHandle {
    #[new]
    fn py_new(world: &TonnerWorldHandle) -> TonnerEntityHandle {
        world.new_entity()
    }
}
