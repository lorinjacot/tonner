use std::{ops::Deref, sync::Arc};

pub use asset::{Asset, AssetManager, import_gltf};

use crate::{Id, storage::DenseEntry};

mod asset;

/// Stuff shared between all scenes
pub struct Resources {
    device: wgpu::Device,
    queue: wgpu::Queue,
    assets: AssetManager,
}

impl Resources {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let assets = AssetManager::new();
        Self {
            device,
            queue,
            assets,
        }
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }
}

/// Immutable resource
#[derive(Clone)]
pub struct Res<T> {
    id: Id<T>,
    value: Arc<(String, T)>,
}

impl<T> Res<T> {
    pub fn name(&self) -> &str {
        &self.value.0
    }

    pub fn id(&self) -> Id<T> {
        self.id
    }
}

impl<T> Deref for Res<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value.1
    }
}

impl<T> DenseEntry for Res<T> {
    type Key = T;
    type Value = (Option<String>, T);

    fn new(id: Id<Self::Key>, value: Self::Value) -> Self {
        Self {
            id,
            value: Arc::new((value.0.unwrap_or_else(|| id.to_string()), value.1)),
        }
    }

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}
