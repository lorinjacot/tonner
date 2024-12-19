use std::{collections::HashMap, ops::Range};

use wgpu::util::DeviceExt;

use super::Asset;

struct PrimitiveManager {
    attributes_buffers: HashMap<usize, wgpu::Buffer>,
    indices_buffers: HashMap<usize, wgpu::Buffer>,
    primitives: Vec<Primitive>,
}

impl PrimitiveManager {
    fn create_attributes_buffer(
        &mut self,
        device: &wgpu::Device,
        buffer_view: &gltf::buffer::View,
        buffers: &Vec<gltf::buffer::Data>,
    ) {
        let buffer_index = buffer_view.buffer().index();
        self.indices_buffers.entry(buffer_index).or_insert_with(|| {
            let offset = buffer_view.offset();
            let contents = &buffers[buffer_index].0[offset..offset + buffer_view.length()];
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: buffer_view.name(),
                contents,
                usage: wgpu::BufferUsages::INDEX,
            })
        });
    }

    fn create_index_buffer(
        &mut self,
        device: &wgpu::Device,
        buffer_view: &gltf::buffer::View,
        buffers: &Vec<gltf::buffer::Data>,
    ) {
        let buffer_index = buffer_view.buffer().index();
        self.inder_buffers.entry(buffer_index).or_insert_with(|| {
            let offset = buffer_view.offset();
            let contents = &buffers[buffer_index].0[offset..offset + buffer_view.length()];
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: buffer_view.name(),
                contents,
                usage: wgpu::BufferUsages::INDEX,
            })
        });
    }
}

struct Primitive {
    topology: wgpu::PrimitiveTopology,
    attributes_buffer_slice: Range<wgpu::BufferAddress>,
    index_buffer_slice: Range<wgpu::BufferAddress>,
}

impl Primitive {
    fn from_gltf(primitive: &gltf::Primitive, asset: &Asset, device: &wgpu::Device) -> Self {
        let topology = match primitive.mode() {
            gltf::mesh::Mode::Points => wgpu::PrimitiveTopology::PointList,
            gltf::mesh::Mode::Lines => wgpu::PrimitiveTopology::LineList,
            gltf::mesh::Mode::LineStrip => wgpu::PrimitiveTopology::LineStrip,
            gltf::mesh::Mode::Triangles => wgpu::PrimitiveTopology::TriangleList,
            gltf::mesh::Mode::TriangleStrip => wgpu::PrimitiveTopology::TriangleStrip,
            _ => panic!("unsupported mesh.primitive.mode (supported: 0,1,3,4,5"),
        };

        let positions = primitive
            .attributes()
            .find(|(s, _)| s == &gltf::Semantic::Positions)
            .expect("should have a position attribute");

        let normals = primitive
            .attributes()
            .find(|(s, _)| s == &gltf::Semantic::Normals)
            .expect("should have a normals attribute");

        // let attributes_buffer_layout = wgpu::VertexBufferLayout {
        //     array_stride:
        // }

        todo!()
    }
}
