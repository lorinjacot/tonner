use std::ops::Range;

use wgpu::util::DeviceExt;

use crate::{Id, Storm, storage::DenseEntry};

pub struct Asset {
    id: Id<Self>,
}

impl Storm {
    pub fn load_gltf(&mut self, path: impl AsRef<std::path::Path>) -> Result<Asset, gltf::Error> {
        let (document, buffers, images) = gltf::import(path)?;

        let mut accessors: Vec<Option<Accessor>> = vec![None; document.accessors().len()];
        let mut views: Vec<Option<View>> = vec![None; document.views().len()];
        let _meshes: Vec<Option<Mesh>> = vec![None; document.meshes().len()];

        for mesh in document.meshes() {
            for primitive in mesh.primitives() {
                if primitive.get(&gltf::Semantic::Positions).is_none() {
                    continue;
                }

                let index_buffer =
                    primitive
                        .indices()
                        .map(|indices| match &accessors[indices.index()] {
                            Some(Accessor::IndexBuffer(index_buffer)) => index_buffer.clone(),
                            None => {
                                let idx = indices.index();
                                let index_buffer =
                                    IndexBuffer::from(indices, &buffers, &mut views, &self.device);
                                accessors[idx] = Some(Accessor::IndexBuffer(index_buffer.clone()));
                                index_buffer
                            }
                        });
            }
        }

        todo!()
    }
}

impl DenseEntry for Asset {
    type Key = Self;
    type Value = ();

    fn new(id: Id<Self::Key>, _value: Self::Value) -> Self {
        Self { id }
    }

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}

#[derive(Clone)]
enum Accessor {
    IndexBuffer(IndexBuffer),
}

#[derive(Debug, Clone)]
struct IndexBuffer {
    buffer: wgpu::Buffer,
    bounds: Range<u64>,
    format: wgpu::IndexFormat,
}

impl IndexBuffer {
    fn from(
        indices: gltf::Accessor,
        buffers: &Vec<gltf::buffer::Data>,
        views: &mut Vec<Option<View>>,
        device: &wgpu::Device,
    ) -> Self {
        if indices.sparse().is_some() {
            todo!("sparse primitive indices")
        } else {
            let view = indices
                .view()
                .expect("dense gltf accessor should have a view");
            let format = match indices.data_type() {
                gltf::accessor::DataType::U16 => wgpu::IndexFormat::Uint16,
                gltf::accessor::DataType::U32 => wgpu::IndexFormat::Uint32,
                _ => panic!("index buffer format should be one of u16 or u32"),
            };
            let view_idx = view.index();
            let buffer = match &views[view_idx] {
                Some(view) => view.buffer.clone(),
                None => {
                    let view = View::from(view, buffers, wgpu::BufferUsages::INDEX, device);
                    let buffer = view.buffer.clone();
                    views[view_idx] = Some(view);
                    buffer
                }
            };
            let start = indices.offset() as u64;
            let end = start + (indices.count() * indices.size()) as u64;
            let bounds = start..end;
            Self {
                buffer,
                bounds,
                format,
            }
        }
    }
}

#[derive(Clone)]
struct View {
    buffer: wgpu::Buffer,
}

impl View {
    fn from(
        view: gltf::buffer::View,
        buffers: &Vec<gltf::buffer::Data>,
        usage: wgpu::BufferUsages,
        device: &wgpu::Device,
    ) -> Self {
        let name = format!("view({}) {}", view.index(), view.name().unwrap_or(""));
        let start = view.offset();
        let end = start + view.length();
        let contents = &buffers[view.buffer().index()].0[start..end];
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&name),
            contents,
            usage,
        });
        Self { buffer }
    }
}

#[derive(Clone)]
struct Mesh {
    primitives: Vec<Primitive>,
}

#[derive(Clone)]
struct Primitive {
    pipeline: wgpu::RenderPipeline,
    index_buffer: Option<IndexBuffer>,
    vertex_buffers: Vec<(wgpu::Buffer, Range<u64>)>,
}
