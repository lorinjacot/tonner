use std::{iter::repeat_n, ops::{Deref, Index}};

use wgpu::util::DeviceExt;

use super::{
    storage::{Id, SparseMap, SparseSet},
    Asset,
};

pub struct BufferManager {
    buffers: SparseSet<Buffer>,
    assets: SparseMap<Asset, AssetData>,
}

impl BufferManager {
    pub fn new() -> Self {
        Self {
            buffers: SparseSet::new(),
            assets: SparseMap::new(),
        }
    }

    pub fn register_asset(&mut self, id: Id<Asset>, buffers: Vec<gltf::buffer::Data>) {
        self.assets.insert(
            id,
            AssetData {
                data: buffers,
                buffer_view_mapping: Vec::new(),
            },
        );
    }

    pub fn load_buffer_view(
        &mut self,
        asset: Id<Asset>,
        buffer_view: gltf::buffer::View,
        usage: wgpu::BufferUsages,
        device: &wgpu::Device,
    ) -> Id<Buffer> {
        match self.assets[asset]
            .buffer_view_mapping
            .get(buffer_view.index())
        {
            Some(Some(id)) => *id,
            _ => self.create_buffer_view(asset, buffer_view, usage, device),
        }
    }

    fn create_buffer_view(
        &mut self,
        asset: Id<Asset>,
        buffer_view: gltf::buffer::View,
        usage: wgpu::BufferUsages,
        device: &wgpu::Device,
    ) -> Id<Buffer> {
        let offset = buffer_view.offset();
        let length = buffer_view.length();

        let asset = &mut self.assets[asset];
        let contents = &asset.data[buffer_view.buffer().index()][offset..offset + length];

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("Buffer view {}", buffer_view.name().unwrap_or(""))),
            contents,
            usage,
        });

        let id = self.buffers.push(Buffer {
            buffer,
            stride: buffer_view.stride(),
        });

        match asset.buffer_view_mapping.get_mut(buffer_view.index()) {
            Some(entry) => *entry = Some(id),
            None => {
                let iter = repeat_n(None, buffer_view.index() - asset.buffer_view_mapping.len())
                    .chain(Some(Some(id)));
                asset.buffer_view_mapping.extend(iter);
            }
        }

        id
    }

    pub fn get(&self, id: Id<Buffer>) -> Option<&Buffer> {
        self.buffers.get(id)
    }
}

impl Index<Id<Buffer>> for BufferManager {
    type Output = Buffer;

    fn index(&self, index: Id<Buffer>) -> &Self::Output {
        &self.buffers[index]
    }
}

pub struct Buffer {
    buffer: wgpu::Buffer,
    stride: Option<usize>,
}

impl Deref for Buffer {
    type Target = wgpu::Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

struct AssetData {
    data: Vec<gltf::buffer::Data>,
    buffer_view_mapping: Vec<Option<Id<Buffer>>>,
}
