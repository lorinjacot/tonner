use std::path::Path;

pub struct Asset {
    pub document: gltf::Document,
    pub buffers: Vec<gltf::buffer::Data>,
    _images: Vec<gltf::image::Data>,
}

impl Asset {
    pub fn load<P: AsRef<Path>>(path: P) -> Self {
        let (document, buffers, _images) = gltf::import(path).unwrap();
        Self {
            document,
            buffers,
            _images,
        }
    }
}

// mod primitive;
// pub mod scene;

// pub struct Asset {
//     gltf_document: gltf::Document,
//     gltf_buffers: Vec<gltf::buffer::Data>,
//     gltf_images: Vec<gltf::image::Data>,
//     meshes: MeshManager,
//     primitive_pipeline: wgpu::RenderPipeline,
// }

// impl Asset {
//     pub fn load<P: AsRef<Path>>(
//         path: P,
//         device: &wgpu::Device,
//         targets: &[Option<wgpu::ColorTargetState>],
//     ) -> Self {
//         let (gltf_document, gltf_buffers, gltf_images) =
//             gltf::import(path).expect("failed to open asset");

//         let nodes = NodeManager::new();
//         let meshes = MeshManager::new(device);

//         let camera_bind_group_layout =
//             device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
//                 label: Some("Primitive camera bind group layout"),
//                 entries: &[wgpu::BindGroupLayoutEntry {
//                     binding: 0,
//                     visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
//                     ty: wgpu::BindingType::Buffer {
//                         ty: wgpu::BufferBindingType::Uniform,
//                         has_dynamic_offset: false,
//                         min_binding_size: None,
//                     },
//                     count: None,
//                 }],
//             });

//         let module = device.create_shader_module(wgpu::include_wgsl!("asset/primitive.wgsl"));

//         let primitive_pipeline_layout =
//             device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
//                 label: Some("Primitive pipeline layout"),
//                 bind_group_layouts: &[
//                     meshes.material_bind_group_layout(),
//                     &camera_bind_group_layout,
//                 ],
//                 push_constant_ranges: &[],
//             });

//         let primitive_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
//             label: Some("Primitive pipeline"),
//             layout: Some(&primitive_pipeline_layout),
//             vertex: wgpu::VertexState {
//                 module: &module,
//                 entry_point: Some("vs_main"),
//                 compilation_options: wgpu::PipelineCompilationOptions::default(),
//                 buffers: &[wgpu::VertexBufferLayout {
//                     array_stride: 3 * 4,
//                     step_mode: wgpu::VertexStepMode::Vertex,
//                     attributes: &[wgpu::VertexAttribute {
//                         format: wgpu::VertexFormat::Float32x3,
//                         offset: 0,
//                         shader_location: 0,
//                     }],
//                 }],
//             },
//             primitive: wgpu::PrimitiveState {
//                 topology: wgpu::PrimitiveTopology::TriangleList,
//                 strip_index_format: None,
//                 front_face: wgpu::FrontFace::Ccw,
//                 cull_mode: None,
//                 unclipped_depth: false,
//                 polygon_mode: wgpu::PolygonMode::Fill,
//                 conservative: false,
//             },
//             depth_stencil: None,
//             multisample: wgpu::MultisampleState {
//                 count: 1,
//                 mask: !0,
//                 alpha_to_coverage_enabled: false,
//             },
//             fragment: Some(wgpu::FragmentState {
//                 module: &module,
//                 entry_point: Some("fs_main"),
//                 compilation_options: wgpu::PipelineCompilationOptions::default(),
//                 targets,
//             }),
//             multiview: None,
//             cache: None,
//         });

//         Self {
//             gltf_document,
//             gltf_buffers,
//             gltf_images,
//             nodes,
//             meshes,
//             primitive_pipeline,
//         }
//     }
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
