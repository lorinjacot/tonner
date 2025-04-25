use crate::storage::SparseSet;

use super::{Res, Resources};

pub struct Asset {
    document: gltf::Document,
    buffers: Vec<gltf::buffer::Data>,
    images: Vec<gltf::image::Data>,
}

pub fn import_gltf<P>(path: P, resources: &mut Resources) -> Result<&mut Res<Asset>, gltf::Error>
where
    P: AsRef<std::path::Path>,
{
    let name = path.as_ref().to_string_lossy().to_string();
    let (document, buffers, images) = gltf::import(path)?;

    Ok(resources.assets.inner.push((
        Some(name),
        Asset {
            document,
            buffers,
            images,
        },
    )))
}

pub struct AssetManager {
    inner: SparseSet<Res<Asset>>,
}

impl AssetManager {
    pub fn new() -> Self {
        Self {
            inner: SparseSet::new(),
        }
    }
}
