use std::sync::{Arc, Mutex};

#[cfg(feature = "python")]
use pyo3::prelude::*;

use crate::{
    Context,
    entity_component::{EntityId, EntityManager},
    scene_graph::SceneGraph,
};

pub trait World {}

#[derive(Debug)]
pub struct TonnerWorld {
    context: Context,
    pub entity_manager: EntityManager,
    pub scene_graph: SceneGraph,
}

impl TonnerWorld {
    pub fn new(context: Context) -> TonnerWorld {
        let scene_graph = SceneGraph::new(&context);
        TonnerWorld {
            context,
            entity_manager: EntityManager::new(),
            scene_graph,
        }
    }

    pub fn context(&self) -> &Context {
        &self.context
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(
    feature = "python",
    pyclass(name = "TonnerWorld", dict, skip_from_py_object, frozen)
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

pub trait Entity {
    fn id(&self) -> EntityId;
}

#[derive(Debug, Clone)]
#[cfg_attr(
    feature = "python",
    pyclass(name = "TonnerEntity", dict, skip_from_py_object)
)]
pub struct TonnerEntityHandle {
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
