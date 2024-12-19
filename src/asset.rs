// use std::{collections::HashMap, path::Path};

// use wgpu::util::DeviceExt;

// // mod mesh;
pub mod primitive;

// pub struct Asset {
//     document: gltf::Document,
//     buffers: Vec<gltf::buffer::Data>,
//     images: Vec<gltf::image::Data>,
//     index_buffers: HashMap<usize, wgpu::Buffer>,
// }

// impl Asset {
//     pub fn load<P: AsRef<Path>>(path: P) -> gltf::Result<Self> {
//         let (document, buffers, images) = gltf::import(path)?;
//         Ok(Self {
//             document,
//             buffers,
//             images,
//             index_buffers: HashMap::new(),
//         })
//     }

//     fn data_from_accessor<'a>(
//         &'a self,
//         accessor: &'a gltf::Accessor,
//     ) -> impl Iterator<Item = &'a [u8]> {
//         let view = accessor.view().expect("sparse accessor are not supported");
//         let data = &self.buffers[view.buffer().index()].0;

//         let chunck_size = accessor.size();
//         let view_offset = view.offset();
//         let stride = view.stride().unwrap_or(chunck_size);

//         data[view_offset + accessor.offset()..view_offset + view.length()]
//             .chunks(stride)
//             .take(accessor.count())
//             .map(move |element| &element[0..chunck_size])
//     }
// }

// trait AssetExt {
//     fn create_index_buffer(
//         &self,
//         buffer_view: &gltf::buffer::View,
//         buffers: &Vec<gltf::buffer::Data>,
//     ) -> wgpu::Buffer;

//     fn create_vertex_buffer(
//         &self,
//         buffer_view: &gltf::buffer::View,
//         buffers: &Vec<gltf::buffer::Data>,
//     ) -> wgpu::Buffer;
// }

// impl AssetExt for wgpu::Device {
//     fn create_index_buffer(
//         &self,
//         buffer_view: &gltf::buffer::View,
//         buffers: &Vec<gltf::buffer::Data>,
//     ) -> wgpu::Buffer {
//         let contents = &buffers[buffer_view.buffer().index()].0;
//         let offset = buffer_view.offset();
//         let contents = &contents[offset..offset + buffer_view.length()];

//         self.create_buffer_init(&wgpu::util::BufferInitDescriptor {
//             label: buffer_view.name(),
//             contents,
//             usage: wgpu::BufferUsages::INDEX,
//         })
//     }

//     fn create_vertex_buffer(
//         &self,
//         buffer_view: &gltf::buffer::View,
//         buffers: &Vec<gltf::buffer::Data>,
//     ) -> wgpu::Buffer {
//         let contents = &buffers[buffer_view.buffer().index()].0;
//         let offset = buffer_view.offset();
//         let contents = &contents[offset..offset + buffer_view.length()];

//         self.create_buffer_init(&wgpu::util::BufferInitDescriptor {
//             label: buffer_view.name(),
//             contents,
//             usage: wgpu::BufferUsages::VERTEX,
//         })
//     }
// }

// #[cfg(test)]
// mod tests {
//     use glam::{vec3, Vec3};

//     use super::*;

//     #[test]
//     fn load_indices() {
//         let asset = Asset::load("assets/Box.glb").unwrap();
//         let accessor = asset.document.accessors().find(|a| a.index() == 0).unwrap();
//         let indices = asset
//             .data_from_accessor(&accessor)
//             .map(|index| u16::from_le_bytes(index.try_into().unwrap()))
//             .collect::<Vec<_>>();

//         assert_eq!(
//             indices,
//             vec![
//                 0, 1, 2, 3, 2, 1, 4, 5, 6, 7, 6, 5, 8, 9, 10, 11, 10, 9, 12, 13, 14, 15, 14, 13,
//                 16, 17, 18, 19, 18, 17, 20, 21, 22, 23, 22, 21
//             ]
//         )
//     }

//     #[test]
//     fn load_positions() {
//         let asset = Asset::load("assets/Box.glb").unwrap();
//         let accessor = asset.document.accessors().find(|a| a.index() == 2).unwrap();
//         let positions = asset
//             .data_from_accessor(&accessor)
//             .map(|bytes| {
//                 Vec3::from_slice(
//                     &bytes
//                         .chunks(4)
//                         .map(|e| f32::from_le_bytes(e.try_into().unwrap()))
//                         .collect::<Vec<_>>(),
//                 )
//             })
//             .collect::<Vec<_>>();

//         assert_eq!(
//             positions,
//             vec![
//                 vec3(-0.5, -0.5, 0.5),
//                 vec3(0.5, -0.5, 0.5),
//                 vec3(-0.5, 0.5, 0.5),
//                 vec3(0.5, 0.5, 0.5),
//                 vec3(0.5, -0.5, 0.5),
//                 vec3(-0.5, -0.5, 0.5),
//                 vec3(0.5, -0.5, -0.5),
//                 vec3(-0.5, -0.5, -0.5),
//                 vec3(0.5, 0.5, 0.5),
//                 vec3(0.5, -0.5, 0.5),
//                 vec3(0.5, 0.5, -0.5),
//                 vec3(0.5, -0.5, -0.5),
//                 vec3(-0.5, 0.5, 0.5),
//                 vec3(0.5, 0.5, 0.5),
//                 vec3(-0.5, 0.5, -0.5),
//                 vec3(0.5, 0.5, -0.5),
//                 vec3(-0.5, -0.5, 0.5),
//                 vec3(-0.5, 0.5, 0.5),
//                 vec3(-0.5, -0.5, -0.5),
//                 vec3(-0.5, 0.5, -0.5),
//                 vec3(-0.5, -0.5, -0.5),
//                 vec3(-0.5, 0.5, -0.5),
//                 vec3(0.5, -0.5, -0.5),
//                 vec3(0.5, 0.5, -0.5)
//             ]
//         )
//     }

//     #[test]
//     fn load_normals() {
//         let asset = Asset::load("assets/Box.glb").unwrap();
//         let accessor = asset.document.accessors().find(|a| a.index() == 1).unwrap();
//         let normals = asset
//             .data_from_accessor(&accessor)
//             .map(|bytes| {
//                 Vec3::from_slice(
//                     &bytes
//                         .chunks(4)
//                         .map(|e| f32::from_le_bytes(e.try_into().unwrap()))
//                         .collect::<Vec<_>>(),
//                 )
//             })
//             .collect::<Vec<_>>();

//         assert_eq!(
//             normals,
//             vec![
//                 vec3(0.0, 0.0, 1.0),
//                 vec3(0.0, 0.0, 1.0),
//                 vec3(0.0, 0.0, 1.0),
//                 vec3(0.0, 0.0, 1.0),
//                 vec3(0.0, -1.0, 0.0),
//                 vec3(0.0, -1.0, 0.0),
//                 vec3(0.0, -1.0, 0.0),
//                 vec3(0.0, -1.0, 0.0),
//                 vec3(1.0, 0.0, 0.0),
//                 vec3(1.0, 0.0, 0.0),
//                 vec3(1.0, 0.0, 0.0),
//                 vec3(1.0, 0.0, 0.0),
//                 vec3(0.0, 1.0, 0.0),
//                 vec3(0.0, 1.0, 0.0),
//                 vec3(0.0, 1.0, 0.0),
//                 vec3(0.0, 1.0, 0.0),
//                 vec3(-1.0, 0.0, 0.0),
//                 vec3(-1.0, 0.0, 0.0),
//                 vec3(-1.0, 0.0, 0.0),
//                 vec3(-1.0, 0.0, 0.0),
//                 vec3(0.0, 0.0, -1.0),
//                 vec3(0.0, 0.0, -1.0),
//                 vec3(0.0, 0.0, -1.0),
//                 vec3(0.0, 0.0, -1.0)
//             ]
//         )
//     }
// }
