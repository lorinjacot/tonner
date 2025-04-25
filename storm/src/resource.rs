use std::{ops::Deref, sync::Arc};

use asset::Asset;

use crate::{Id, storage::SparseSet};

mod asset;

/// Stuff shared between all scenes
pub struct Resources {
    device: wgpu::Device,
    queue: wgpu::Queue,
    assets: SparseSet<(Id<Asset>, Asset)>,
}

impl Resources {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let assets = SparseSet::new();

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
pub struct Res<V, K = V> {
    id: Id<K>,
    name: String,
    value: Arc<V>,
}

impl<V, K> Res<V, K> {
    pub fn id(&self) -> Id<K> {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl<V, K> Deref for Res<V, K> {
    type Target = V;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
