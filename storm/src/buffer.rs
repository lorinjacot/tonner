use std::{
    iter::{once, repeat_n},
    ops::{Deref, Index, Range},
};

use wgpu::util::DeviceExt;

use super::{
    storage::{Id, SparseMap, SparseSet},
    Asset,
};

pub struct BufferManager {
    buffers: SparseSet<Buffer>,
    accessors: SparseSet<Accessor>,
    assets: SparseMap<Asset, AssetData>,
}

impl BufferManager {
    pub fn new() -> Self {
        Self {
            buffers: SparseSet::new(),
            accessors: SparseSet::new(),
            assets: SparseMap::new(),
        }
    }

    pub fn register_asset(&mut self, id: Id<Asset>, buffers: Vec<gltf::buffer::Data>) {
        self.assets.insert(
            id,
            AssetData {
                data: buffers,
                buffer_view_mapping: Vec::new(),
                accessor_mapping: Vec::new(),
            },
        );
    }

    pub fn create_buffer(&mut self, buffer: wgpu::Buffer, stride: u64) -> Id<Buffer> {
        self.buffers.push(Buffer { buffer, stride })
    }

    pub fn create_accessor(
        &mut self,
        buffer: Id<Buffer>,
        start: u64,
        end: u64,
        data_type: gltf::accessor::DataType,
        normalized: bool,
        dimensions: gltf_json::accessor::Type,
    ) -> Id<Accessor> {
        self.accessors.push(Accessor {
            buffer,
            start,
            end,
            data_type,
            normalized,
            dimensions,
        })
    }

    pub fn load_buffer_view(
        &mut self,
        asset: Id<Asset>,
        buffer_view: gltf::buffer::View,
        default_stride: u64,
        usage: wgpu::BufferUsages,
        device: &wgpu::Device,
    ) -> Id<Buffer> {
        match self.assets[asset]
            .buffer_view_mapping
            .get(buffer_view.index())
        {
            Some(Some(id)) => *id,
            _ => self.create_buffer_view(asset, buffer_view, default_stride, usage, device),
        }
    }

    fn create_buffer_view(
        &mut self,
        asset: Id<Asset>,
        buffer_view: gltf::buffer::View,
        default_stride: u64,
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
            stride: buffer_view
                .stride()
                .map(|value| value as u64)
                .unwrap_or(default_stride),
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

    pub fn load_accessor(
        &mut self,
        asset: Id<Asset>,
        accessor: gltf::Accessor,
        usage: wgpu::BufferUsages,
        device: &wgpu::Device,
    ) -> Id<Accessor> {
        match self
            .assets
            .entry(asset)
            .or_default()
            .accessor_mapping
            .get(accessor.index())
        {
            Some(Some(id)) => *id,
            _ => self.create_gltf_accessor(asset, accessor, usage, device),
        }
    }

    fn create_gltf_accessor(
        &mut self,
        asset: Id<Asset>,
        accessor: gltf::Accessor,
        usage: wgpu::BufferUsages,
        device: &wgpu::Device,
    ) -> Id<Accessor> {
        let buffer = self.load_buffer_view(
            asset,
            accessor.view().expect("Sparse accessor not supported"),
            accessor.size() as u64,
            usage,
            device,
        );

        let start = accessor.offset() as u64;
        let end = start + accessor.count() as u64 * accessor.size() as u64;
        let id = self.accessors.push(Accessor {
            buffer,
            start,
            end,
            data_type: accessor.data_type(),
            normalized: accessor.normalized(),
            dimensions: accessor.dimensions(),
        });

        let mapping = &mut self.assets[asset].accessor_mapping;
        match mapping.get_mut(accessor.index()) {
            Some(entry) => *entry = Some(id),
            None => {
                let iter = repeat_n(None, accessor.index() - mapping.len()).chain(once(Some(id)));
                mapping.extend(iter);
            }
        }

        id
    }

    pub fn buffer_data(&self, asset: Id<Asset>) -> Option<&Vec<gltf::buffer::Data>> {
        self.assets.get(asset).map(|asset| &asset.data)
    }
}

impl Index<Id<Buffer>> for BufferManager {
    type Output = Buffer;

    fn index(&self, index: Id<Buffer>) -> &Self::Output {
        &self.buffers[index]
    }
}

impl Index<Id<Accessor>> for BufferManager {
    type Output = Accessor;

    fn index(&self, index: Id<Accessor>) -> &Self::Output {
        &self.accessors[index]
    }
}

pub struct Buffer {
    buffer: wgpu::Buffer,
    stride: u64,
}

impl Buffer {
    pub fn stride(&self) -> u64 {
        self.stride
    }
}

impl Deref for Buffer {
    type Target = wgpu::Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

pub struct Accessor {
    buffer: Id<Buffer>,
    start: u64,
    end: u64,
    data_type: gltf::accessor::DataType,
    normalized: bool,
    dimensions: gltf::accessor::Dimensions,
}

impl Accessor {
    pub fn buffer(&self) -> Id<Buffer> {
        self.buffer
    }

    pub fn vertex_attribute_layout(&self, shader_location: u32) -> wgpu::VertexAttribute {
        use gltf::accessor::DataType::*;
        use gltf::accessor::Dimensions::*;
        use wgpu::VertexFormat::*;

        let format = match (self.data_type, self.normalized, self.dimensions) {
            (U8, false, Scalar) => Uint8,
            (U8, false, Vec2) => Uint8x2,
            (U8, false, Vec4) => Uint8x4,
            (I8, false, Scalar) => Sint8,
            (I8, false, Vec2) => Sint8x2,
            (I8, false, Vec4) => Sint8x4,
            (U8, true, Scalar) => Unorm8,
            (U8, true, Vec2) => Unorm8x2,
            (U8, true, Vec4) => Unorm8x4,
            (I8, true, Scalar) => Snorm8,
            (I8, true, Vec2) => Snorm8x2,
            (I8, true, Vec4) => Snorm8x4,
            (U16, false, Scalar) => Uint16,
            (U16, false, Vec2) => Uint16x2,
            (U16, false, Vec4) => Uint16x4,
            (I16, false, Scalar) => Sint16,
            (I16, false, Vec2) => Sint16x2,
            (I16, false, Vec4) => Sint16x4,
            (U16, true, Scalar) => Unorm16,
            (U16, true, Vec2) => Unorm16x2,
            (U16, true, Vec4) => Unorm16x4,
            (I16, true, Scalar) => Snorm16,
            (I16, true, Vec2) => Snorm16x2,
            (I16, true, Vec4) => Snorm16x4,
            (F32, _, Scalar) => Float32,
            (F32, _, Vec2) => Float32x2,
            (F32, _, Vec3) => Float32x3,
            (F32, _, Vec4) => Float32x4,
            (U32, false, Scalar) => Uint32,
            (U32, false, Vec2) => Uint32x2,
            (U32, false, Vec3) => Uint32x3,
            (U32, false, Vec4) => Uint32x4,
            (_, _, Mat2 | Mat3 | Mat4) => {
                panic!("Mat2, Mat3 and Mat4 vertex attribute unsupported")
            }
            (U32, true, _) => panic!("u32 normalized vertex attribute unsupported"),
            (U8 | I8 | U16 | I16, _, Vec3) => {
                panic!("Vec3 of u8, i8, u16 and i16 vertex attribute unsupported")
            }
        };

        wgpu::VertexAttribute {
            format,
            offset: self.start,
            shader_location,
        }
    }

    pub fn index_format(&self) -> wgpu::IndexFormat {
        match self.data_type {
            gltf::accessor::DataType::U16 => wgpu::IndexFormat::Uint16,
            gltf::accessor::DataType::U32 => wgpu::IndexFormat::Uint32,
            _ => panic!("unsupported index format"),
        }
    }

    pub fn bounds(&self) -> Range<u64> {
        self.start..self.end
    }
}

#[derive(Default)]
struct AssetData {
    data: Vec<gltf::buffer::Data>,
    buffer_view_mapping: Vec<Option<Id<Buffer>>>,
    accessor_mapping: Vec<Option<Id<Accessor>>>,
}
