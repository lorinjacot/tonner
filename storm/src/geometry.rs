use std::{borrow::Cow, f32::consts::PI, num::NonZeroU32};

use bitflags::bitflags;
use bytemuck::cast_slice;
use glam::vec3;
use wgpu::util::DeviceExt;

use crate::{DenseEntry, Id, Resources, storage::SetEntry};

pub struct Geometry {
    id: Id<Self>,
    vertex_buffers: Vec<wgpu::Buffer>,
    vertex_buffer_layouts: Vec<VertexBufferLayout>,
    attributes: Attributes,
}

pub struct GeometryDescriptor {
    vertex_buffers: Vec<wgpu::Buffer>,
    vertex_buffer_layouts: Vec<VertexBufferLayout>,
    attributes: Attributes,
}

impl DenseEntry for Geometry {
    type Key = Self;

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}

impl SetEntry for Geometry {
    type Descriptor = GeometryDescriptor;

    fn new(id: Id<Self::Key>, desc: Self::Descriptor) -> Self {
        Self {
            id,
            vertex_buffers: desc.vertex_buffers,
            vertex_buffer_layouts: desc.vertex_buffer_layouts,
            attributes: desc.attributes,
        }
    }
}

pub struct GeometryBuilder<'a, 'r> {
    resources: &'r mut Resources,
    vertex_count: Option<NonZeroU32>,
    indices: Indices<'a>,
    positions: Option<Cow<'a, [[f32; 3]]>>,
    normals: Option<Cow<'a, [[f32; 3]]>>,
    tex_coords: Vec<(Attribute, Cow<'a, [[f32; 2]]>)>,
    attributes: Attributes,
}

impl<'a, 'r> GeometryBuilder<'a, 'r> {
    pub fn new(resources: &'r mut Resources) -> Self {
        Self {
            resources,
            vertex_count: None,
            indices: Indices::None,
            positions: None,
            normals: None,
            tex_coords: Vec::new(),
            attributes: Attributes::empty(),
        }
    }

    pub fn vertex_count(mut self, vertex_count: u32) -> Self {
        self.vertex_count = NonZeroU32::new(vertex_count);
        self
    }

    pub fn indices_u16(mut self, indices: Cow<'a, [u16]>) -> Self {
        self.indices = Indices::U16(indices);
        self
    }

    pub fn indices_u32(mut self, indices: Cow<'a, [u32]>) -> Self {
        self.indices = Indices::U32(indices);
        self
    }

    pub fn positions(mut self, positions: Cow<'a, [[f32; 3]]>) -> Self {
        self.positions = Some(positions);
        self.attributes.insert(Attributes::POSITION);
        self
    }

    pub fn normals(mut self, normals: Cow<'a, [[f32; 3]]>) -> Self {
        self.normals = Some(normals);
        self.attributes.insert(Attributes::NORMAL);
        self
    }

    pub fn tex_coords(mut self, set: u32, tex_coords: Cow<'a, [[f32; 2]]>) -> Self {
        let (attribute, flag) = match set {
            0 => (Attribute::TexCoord0, Attributes::TEX_COORD_0),
            1 => (Attribute::TexCoord1, Attributes::TEX_COORD_1),
            _ => panic!("unsupported texure coordinate set"),
        };
        self.tex_coords.push((attribute, tex_coords));
        self.attributes.insert(flag);
        self
    }

    /// An helper for generating sphere geometries.
    /// Based on [three.js/SphereGeometry](https://threejs.org/docs/#api/en/geometries/SphereGeometry).
    ///
    /// The geometry is created by sweeping and calculating vertexes around the Y axis (horizontal sweep)
    /// and the Z axis (vertical sweep). Thus, incomplete spheres (akin to 'sphere slices') can be created
    /// through the use of different values of `phi_start`, `phi_length`, `theta_start` and `theta_length`,
    /// in order to define the points in which we start (or end) calculating those vertices.
    pub fn sphere(self, desc: &'a SphereDescriptor) -> Self {
        let width_segments = desc.width_segments.max(3);
        let height_segments = desc.height_segments.max(2);

        let theta_end = (desc.theta_start + desc.theta_length).min(PI);

        let row_count = height_segments + 1;
        let col_count = width_segments + 1;
        let mut index = 0u32;
        let mut grid = Vec::with_capacity(row_count);

        let vertex_count = (height_segments + 1) * (width_segments + 1);
        let mut positions = Vec::with_capacity(vertex_count);
        let mut normals = Vec::with_capacity(vertex_count);
        let mut uvs = Vec::with_capacity(vertex_count);

        for y in 0..=height_segments {
            let mut vertices_row = Vec::with_capacity(col_count);
            let v = y as f32 / height_segments as f32;

            // special case for the poles
            let u_offset = if y == 0 && desc.theta_start == 0.0 {
                0.5 / width_segments as f32
            } else if y == height_segments && theta_end == PI {
                -0.5 / width_segments as f32
            } else {
                0.0
            };

            for x in 0..=width_segments {
                let u = x as f32 / width_segments as f32;

                let phi = desc.phi_start + u * desc.phi_length;
                let theta = desc.theta_start + v * desc.theta_length;

                let vertex = vec3(
                    -desc.radius * phi.cos() * theta.sin(),
                    desc.radius * theta.cos(),
                    desc.radius * phi.sin() * theta.sin(),
                );

                positions.push(vertex.to_array());
                normals.push(vertex.normalize().to_array());
                uvs.push([u + u_offset, 1.0 - v]);

                vertices_row.push(index);
                index += 1;
            }

            grid.push(vertices_row);
        }

        let mut index_count = (height_segments - 1) * width_segments * 6;
        if desc.theta_start > 0.0 {
            index_count += width_segments * 3;
        }
        if theta_end < PI {
            index_count += width_segments * 3;
        }
        let mut indices = Vec::with_capacity(index_count);

        for y in 0..height_segments {
            for x in 0..width_segments {
                let a = grid[y][x + 1];
                let b = grid[y][x];
                let c = grid[y + 1][x];
                let d = grid[y + 1][x + 1];

                if y != 0 || desc.theta_start > 0.0 {
                    indices.extend_from_slice(&[a, b, d]);
                }
                if y != height_segments - 1 || theta_end < PI {
                    indices.extend_from_slice(&[b, c, d]);
                }
            }
        }

        self.vertex_count(indices.len() as u32)
            .indices_u32(indices.into())
            .positions(positions.into())
            .normals(normals.into())
            .tex_coords(0, uvs.into())
    }

    pub fn build(self, _encoder: &mut wgpu::CommandEncoder) -> &'r mut Geometry {
        let mut desc = GeometryDescriptor {
            vertex_buffers: Vec::new(),
            vertex_buffer_layouts: Vec::new(),
            attributes: self.attributes,
        };

        let mut create_vertex_buffer =
            |name, contents, array_stride, format, attribute: Attribute| {
                desc.vertex_buffers
                    .push(self.resources.device.create_buffer_init(
                        &wgpu::util::BufferInitDescriptor {
                            label: Some(name),
                            contents,
                            usage: wgpu::BufferUsages::VERTEX,
                        },
                    ));
                desc.vertex_buffer_layouts.push(VertexBufferLayout {
                    array_stride,
                    attributes: vec![wgpu::VertexAttribute {
                        format,
                        offset: 0,
                        shader_location: attribute as u32,
                    }],
                });
            };

        let positions = self.positions.expect("positions attribute should be set");
        create_vertex_buffer(
            "Positions buffer",
            cast_slice(&positions),
            3 * 4,
            wgpu::VertexFormat::Float32x3,
            Attribute::Position,
        );

        let normals = match self.normals {
            Some(normals) => normals,
            None => todo!("compute normals"),
        };
        create_vertex_buffer(
            "Normals buffer",
            cast_slice(&normals),
            3 * 4,
            wgpu::VertexFormat::Float32x3,
            Attribute::Normal,
        );

        for (attribute, tex_coords) in &self.tex_coords {
            create_vertex_buffer(
                "Texture coordinate buffer",
                cast_slice(tex_coords),
                2 * 4,
                wgpu::VertexFormat::Float32x2,
                *attribute,
            );
        }

        let mut check_tex_coord = |attribute: Attribute, flag: Attributes| {
            if !desc.attributes.contains(flag) {
                desc.vertex_buffers.push(
                    self.resources
                        .geometry_builder_data
                        .dummy_tex_coord_buffer
                        .clone(),
                );
                desc.vertex_buffer_layouts.push(VertexBufferLayout {
                    array_stride: 0,
                    attributes: vec![wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Unorm8x2,
                        offset: 0,
                        shader_location: attribute as u32,
                    }],
                });
            }
        };
        check_tex_coord(Attribute::TexCoord0, Attributes::TEX_COORD_0);
        check_tex_coord(Attribute::TexCoord1, Attributes::TEX_COORD_1);

        self.resources.geometries.push(desc)
    }
}

pub(super) struct GeometryBuilderData {
    dummy_tex_coord_buffer: wgpu::Buffer,
}

impl GeometryBuilderData {
    pub fn new(device: &wgpu::Device) -> Self {
        let dummy_tex_coord_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dummy texture coordinate buffer"),
            size: 2,
            usage: wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        Self {
            dummy_tex_coord_buffer,
        }
    }
}

/// An helper for generating sphere geometries.
/// Based on [three.js/SphereGeometry](https://threejs.org/docs/#api/en/geometries/SphereGeometry).
pub struct SphereDescriptor {
    /// Sphere radius. Default is `1.0`.
    pub radius: f32,
    /// Number of horizontal segments. Minimum value is `3`, and the default is `32`.
    pub width_segments: usize,
    /// Number of vertical segments. Minimum value is `2`, and the default is `16`.
    pub height_segments: usize,
    /// Specify horizontal starting angle. Default is `0.0`.
    pub phi_start: f32,
    /// Specify horizontal sweep angle size. Default is `2.0 * PI`.
    pub phi_length: f32,
    /// Specify vertical starting angle. Default is `0.0`.
    pub theta_start: f32,
    /// Specify vertical sweep angle size. Default is `PI`.
    pub theta_length: f32,
}

impl Default for SphereDescriptor {
    fn default() -> Self {
        Self {
            radius: 1.0,
            width_segments: 32,
            height_segments: 16,
            phi_start: 0.0,
            phi_length: 2.0 * PI,
            theta_start: 0.0,
            theta_length: PI,
        }
    }
}

struct VertexBufferLayout {
    array_stride: wgpu::BufferAddress,
    attributes: Vec<wgpu::VertexAttribute>,
}

enum Indices<'a> {
    None,
    U16(Cow<'a, [u16]>),
    U32(Cow<'a, [u32]>),
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(usize)]
enum Attribute {
    Position = 0,
    Normal = 1,
    TexCoord0 = 2,
    TexCoord1 = 3,
}

bitflags! {
    struct Attributes: u8 {
        const POSITION = 1 << 0;
        const NORMAL = 1 << 1;
        const TEX_COORD_0 = 1 << 2;
        const TEX_COORD_1 = 1 << 3;
    }
}
