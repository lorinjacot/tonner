use std::path::Path;

use crate::storage::{Id, Storage};

use super::Scene;

pub struct AssetManager {
    assets: Storage<Asset>,
}

impl AssetManager {
    pub fn new() -> Self {
        let assets = Storage::new();
        AssetManager { assets }
    }

    pub fn load(&mut self, path: impl AsRef<Path>) -> Result<AssetId, gltf::Error> {
        let (document, buffers, images) = gltf::import(path)?;
        Ok(self.assets.add(Asset {
            document,
            buffers,
            images,
        }))
    }
}

pub type AssetId = Id<Asset>;
pub struct Asset {
    document: gltf::Document,
    buffers: Vec<gltf::buffer::Data>,
    images: Vec<gltf::image::Data>,
}
