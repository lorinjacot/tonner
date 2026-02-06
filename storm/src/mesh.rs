use std::ops::Deref;

use dashmap::DashMap;

use crate::{
    environment::PREFILTER_MAP_MIP_COUNT,
    geometry::GeometryFlags,
    mesh::{
        instance::PrimitiveInstanceVertex,
        material::{AlphaMode, MaterialFlags},
    },
};

pub use asset::{Mesh, MeshBuilder, MeshBuilderError, MeshId};
pub use instance::{MeshInstance, MeshInstanceId};

pub(crate) use instance::PrimitiveRenderer;

mod asset;
mod instance;
pub mod material;

#[derive(Debug, Clone)]
pub(crate) struct MeshContext {
    primitive_shader_module: wgpu::ShaderModule,
    primitive_pipeline_layout: wgpu::PipelineLayout,
    primitive_bind_group_layout: wgpu::BindGroupLayout,
    primitive_pipelines: DashMap<PrimitivePipelineParameters, [wgpu::RenderPipeline; 2]>,
}

impl MeshContext {
    pub(crate) fn new(
        render_bind_group_layout: &wgpu::BindGroupLayout,
        device: &wgpu::Device,
    ) -> Self {
        let primitive_shader_module =
            device.create_shader_module(wgpu::include_wgsl!("mesh/primitive.wgsl"));

        let primitive_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Primitive bind group layout"),
                entries: &[
                    // geometry
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // material uniform
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // base color texture view
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // base color texture sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // metallic roughness texture view
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // metallic roughness texture sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // normal texture view
                    wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // normal texture sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 7,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // occlusion texture view
                    wgpu::BindGroupLayoutEntry {
                        binding: 8,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // occlusion texture sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 9,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // emissive texture view
                    wgpu::BindGroupLayoutEntry {
                        binding: 10,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // emissive texture sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 11,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let primitive_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Primitive render pipeline layout"),
                bind_group_layouts: &[render_bind_group_layout, &primitive_bind_group_layout],
                push_constant_ranges: &[],
            });

        Self {
            primitive_shader_module,
            primitive_pipeline_layout,
            primitive_bind_group_layout,
            primitive_pipelines: DashMap::new(),
        }
    }

    const ACCUMULATION_BLEND: wgpu::BlendComponent = wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    };
    const REVEALAGE_BLEND: wgpu::BlendComponent = wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Zero,
        dst_factor: wgpu::BlendFactor::OneMinusSrc,
        operation: wgpu::BlendOperation::Add,
    };

    fn get_or_create_render_pipeline(
        &self,
        parameters: PrimitivePipelineParameters,
        device: &wgpu::Device,
    ) -> impl Deref<Target = [wgpu::RenderPipeline; 2]> {
        self.primitive_pipelines
            .entry(parameters.clone())
            .or_insert_with(|| {
                let module = &self.primitive_shader_module;

                let constants = &[
                    ("geometry_flags", parameters.geometry_flags.bits() as f64),
                    ("material_flags", parameters.material_flags.bits() as f64),
                    ("alpha_mode", parameters.alpha_mode as u32 as f64),
                    (
                        "max_prefilter_map_mip",
                        (PREFILTER_MAP_MIP_COUNT - 1) as f64,
                    ),
                ];

                let cull_mode = if parameters.double_sided {
                    None
                } else {
                    Some(wgpu::Face::Back)
                };

                let depth_write_enabled = match parameters.alpha_mode {
                    AlphaMode::Opaque | AlphaMode::Mask => true,
                    AlphaMode::Blend => false,
                };

                let mut desc = wgpu::RenderPipelineDescriptor {
                    label: Some("Primitive (normal) render pass"),
                    layout: Some(&self.primitive_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module,
                        entry_point: Some("vs_main"),
                        compilation_options: wgpu::PipelineCompilationOptions {
                            constants,
                            zero_initialize_workgroup_memory: true,
                        },
                        buffers: &[wgpu::VertexBufferLayout {
                            array_stride: size_of::<PrimitiveInstanceVertex>() as u64,
                            step_mode: wgpu::VertexStepMode::Instance,
                            attributes: &wgpu::vertex_attr_array![
                                0 => Float32x4,
                                1 => Float32x4,
                                2 => Float32x4,
                                3 => Float32x4,
                                4 => Float32x4,
                                5 => Float32x4,
                                6 => Uint32,
                            ],
                        }],
                    },
                    primitive: wgpu::PrimitiveState {
                        topology: parameters.topology,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode,
                        unclipped_depth: false,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        conservative: false,
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: wgpu::TextureFormat::Depth24Plus,
                        depth_write_enabled,
                        depth_compare: wgpu::CompareFunction::Less,
                        stencil: wgpu::StencilState::default(),
                        bias: wgpu::DepthBiasState::default(),
                    }),
                    multisample: wgpu::MultisampleState {
                        count: 1,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    fragment: Some(wgpu::FragmentState {
                        module,
                        entry_point: Some("fs_main"),
                        compilation_options: wgpu::PipelineCompilationOptions {
                            constants,
                            zero_initialize_workgroup_memory: true,
                        },
                        targets: &[
                            Some(wgpu::ColorTargetState {
                                format: wgpu::TextureFormat::Rgba16Float,
                                blend: None,
                                write_mask: wgpu::ColorWrites::all(),
                            }),
                            Some(wgpu::ColorTargetState {
                                format: wgpu::TextureFormat::Rgba16Float,
                                blend: Some(wgpu::BlendState {
                                    color: Self::ACCUMULATION_BLEND,
                                    alpha: Self::ACCUMULATION_BLEND,
                                }),
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                            Some(wgpu::ColorTargetState {
                                format: wgpu::TextureFormat::R8Unorm,
                                blend: Some(wgpu::BlendState {
                                    color: Self::REVEALAGE_BLEND,
                                    alpha: Self::REVEALAGE_BLEND,
                                }),
                                write_mask: wgpu::ColorWrites::ALL,
                            }),
                        ],
                    }),
                    multiview: None,
                    cache: None,
                };

                let normal = device.create_render_pipeline(&desc);

                desc.primitive.front_face = wgpu::FrontFace::Cw;
                let mirrored = device.create_render_pipeline(&desc);

                [normal, mirrored]
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PrimitivePipelineParameters {
    geometry_flags: GeometryFlags,
    topology: wgpu::PrimitiveTopology,
    material_flags: MaterialFlags,
    alpha_mode: AlphaMode,
    double_sided: bool,
}
