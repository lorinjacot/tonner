use std::{
    borrow::Cow,
    f32::consts::PI,
    iter::repeat_n,
    ops::{Index, IndexMut},
};

use bitflags::bitflags;
use bytemuck::cast_slice;
use glam::{Vec3, vec3};
use wgpu::util::DeviceExt;

use crate::{
    DenseEntry, GeometryBuilderTrait, GeometryManagerTrait, GeometryTrait, Id, IndexBuffer,
    ResourcesTrait, StormTrait,
    storage::{IntoIter, Iter, IterMut, SparseSet},
};

pub struct Geometry {
    id: Id<Self>,
    indices: Option<IndexBuffer>,
    vertex_buffers: Vec<wgpu::Buffer>,
    vertex_buffer_layouts: Vec<VertexBufferLayout>,
    vertex_count: u32,
}

impl DenseEntry for Geometry {
    type Key = Self;

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}

impl<Storm> GeometryTrait<Storm> for Geometry
where
    Storm: StormTrait<Geometry = Geometry>,
{
    fn indices(&self) -> &Option<crate::IndexBuffer> {
        &self.indices
    }

    fn vertex_buffer(&self) -> &[wgpu::Buffer] {
        &self.vertex_buffers
    }

    fn vertex_buffer_layouts(
        &self,
    ) -> impl Iterator<Item = wgpu::VertexBufferLayout> + ExactSizeIterator {
        self.vertex_buffer_layouts
            .iter()
            .map(|layout| wgpu::VertexBufferLayout {
                array_stride: layout.array_stride,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &layout.attributes,
            })
    }

    fn vertex_count(&self) -> u32 {
        self.vertex_count
    }
}

pub struct GeometryManager<Storm>
where
    Storm: StormTrait<GeometryManager = Self>,
{
    geometries: SparseSet<Storm::Geometry>,
    dummy_tex_coords: DummyVertexBuffer,
    dummy_colors: DummyVertexBuffer,
}

impl<Storm> Index<Id<Storm::Geometry>> for GeometryManager<Storm>
where
    Storm: StormTrait<GeometryManager = Self>,
{
    type Output = Storm::Geometry;

    fn index(&self, index: Id<Storm::Geometry>) -> &Self::Output {
        &self.geometries[index]
    }
}

impl<Storm> IndexMut<Id<Storm::Geometry>> for GeometryManager<Storm>
where
    Storm: StormTrait<GeometryManager = Self>,
{
    fn index_mut(&mut self, index: Id<Storm::Geometry>) -> &mut Self::Output {
        &mut self.geometries[index]
    }
}

impl<Storm> IntoIterator for GeometryManager<Storm>
where
    Storm: StormTrait<GeometryManager = Self>,
{
    type Item = Storm::Geometry;
    type IntoIter = IntoIter<Storm::Geometry>;

    fn into_iter(self) -> Self::IntoIter {
        self.geometries.into_iter()
    }
}

impl<Storm> crate::Manager<Storm::Geometry> for GeometryManager<Storm>
where
    Storm: StormTrait<GeometryManager = Self>,
{
    type Iter<'a> = Iter<'a, Storm::Geometry>;
    type IterMut<'a> = IterMut<'a, Storm::Geometry>;

    fn get(&self, id: Id<Storm::Geometry>) -> Option<&Storm::Geometry> {
        self.geometries.get(id)
    }

    fn get_mut(&mut self, id: Id<Storm::Geometry>) -> Option<&mut Storm::Geometry> {
        self.geometries.get_mut(id)
    }

    fn iter(&self) -> Self::Iter<'_> {
        self.geometries.iter()
    }

    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.geometries.iter_mut()
    }
}

impl<Storm> GeometryManagerTrait<Storm> for GeometryManager<Storm>
where
    Storm: StormTrait<Geometry = Geometry, GeometryManager = Self>,
{
    fn new(device: &wgpu::Device) -> Self {
        let dummy_tex_coords = DummyVertexBuffer {
            buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Dummy vertex texture coordinate buffer"),
                contents: &[0; 2],
                usage: wgpu::BufferUsages::VERTEX,
            }),
            format: wgpu::VertexFormat::Unorm8x2,
        };

        let dummy_colors = DummyVertexBuffer {
            buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Dummy vertex color buffer"),
                contents: &[u8::MAX; 4],
                usage: wgpu::BufferUsages::VERTEX,
            }),
            format: wgpu::VertexFormat::Unorm8x4,
        };

        Self {
            geometries: SparseSet::new(),
            dummy_tex_coords,
            dummy_colors,
        }
    }
}

#[must_use]
pub struct GeometryBuilder<'a, 'r, Storm>
where
    Storm: StormTrait,
{
    resources: &'r mut Storm::Resources,
    indices: Indices<'a>,
    positions: Option<Cow<'a, [[f32; 3]]>>,
    normals: Option<Cow<'a, [[f32; 3]]>>,
    tex_coords: Vec<(Attribute, TexCoords<'a>)>,
    colors: Vec<(Attribute, Colors<'a>)>,
    attributes: Attributes,
}

impl<'a, 'r, Storm> GeometryBuilder<'a, 'r, Storm>
where
    Storm: StormTrait,
{
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

    fn tex_coords(mut self, set: u32, tex_coords: TexCoords<'a>) -> Self {
        let (attribute, flag) = match set {
            0 => (Attribute::TexCoord0, Attributes::TEX_COORD_0),
            1 => (Attribute::TexCoord1, Attributes::TEX_COORD_1),
            _ => panic!("unsupported vertex texure coordinate set"),
        };
        self.tex_coords.push((attribute, tex_coords));
        self.attributes.insert(flag);
        self
    }

    pub fn tex_coords_u8(self, set: u32, tex_coords: Cow<'a, [[u8; 2]]>) -> Self {
        self.tex_coords(set, TexCoords::U8(tex_coords))
    }

    pub fn tex_coords_u16(self, set: u32, tex_coords: Cow<'a, [[u16; 2]]>) -> Self {
        self.tex_coords(set, TexCoords::U16(tex_coords))
    }

    pub fn tex_coords_f32(self, set: u32, tex_coords: Cow<'a, [[f32; 2]]>) -> Self {
        self.tex_coords(set, TexCoords::F32(tex_coords))
    }

    fn colors(mut self, set: u32, colors: Colors<'a>) -> Self {
        let (attribute, flag) = match set {
            0 => (Attribute::Color0, Attributes::COLOR_0),
            _ => panic!("unsupported vertex color set"),
        };
        self.colors.push((attribute, colors));
        self.attributes.insert(flag);
        self
    }

    pub fn colors_u8(self, set: u32, colors: Cow<'a, [[u8; 4]]>) -> Self {
        self.colors(set, Colors::RgbaU8(colors))
    }

    pub fn colors_u16(self, set: u32, colors: Cow<'a, [[u16; 4]]>) -> Self {
        self.colors(set, Colors::RgbaU16(colors))
    }

    pub fn colors_f32(self, set: u32, colors: Cow<'a, [[f32; 4]]>) -> Self {
        self.colors(set, Colors::RgbaF32(colors))
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

        self.indices_u32(indices.into())
            .positions(positions.into())
            .normals(normals.into())
            .tex_coords_f32(0, uvs.into())
    }
}

impl<'a, 'r, Storm> GeometryBuilderTrait<'a, 'r, Storm> for GeometryBuilder<'a, 'r, Storm>
where
    Storm: StormTrait<
            Geometry = Geometry,
            GeometryManager = GeometryManager<Storm>,
            GeometryBuilder<'a, 'r> = Self,
        >,
{
    fn new(resources: &'r mut Storm::Resources, _encoder: &'a mut wgpu::CommandEncoder) -> Self {
        Self {
            resources,
            indices: Indices::None,
            positions: None,
            normals: None,
            tex_coords: Vec::new(),
            colors: Vec::new(),
            attributes: Attributes::empty(),
        }
    }

    fn build(mut self) -> &'r mut Storm::Geometry {
        let mut vertex_buffers = Vec::new();
        let mut vertex_buffer_layouts = Vec::new();

        let mut create_vertex_buffer =
            |name, contents, array_stride, format, attribute: Attribute| {
                vertex_buffers.push(self.resources.device().create_buffer_init(
                    &wgpu::util::BufferInitDescriptor {
                        label: Some(name),
                        contents,
                        usage: wgpu::BufferUsages::VERTEX,
                    },
                ));
                vertex_buffer_layouts.push(VertexBufferLayout {
                    array_stride,
                    attributes: vec![wgpu::VertexAttribute {
                        format,
                        offset: 0,
                        shader_location: attribute as u32,
                    }],
                });
            };

        let mut positions = self.positions.expect("positions attribute should be set");
        let mut vertex_count = positions.len();

        let normals = match self.normals {
            Some(normals) => normals,
            None => {
                let indices: Option<Vec<usize>> = match self.indices {
                    Indices::None => None,
                    Indices::U16(slice) => {
                        Some(slice.iter().map(|index| *index as usize).collect())
                    }
                    Indices::U32(slice) => {
                        Some(slice.iter().map(|index| *index as usize).collect())
                    }
                };
                self.indices = Indices::None;
                if let Some(indices) = indices {
                    vertex_count = indices.len();
                    positions = indices.iter().map(|index| positions[*index]).collect();
                    for (_, tex_coords) in self.tex_coords.iter_mut() {
                        *tex_coords = tex_coords.to_unindexed(&indices);
                    }
                    for (_, colors) in self.colors.iter_mut() {
                        *colors = colors.to_unindexed(&indices);
                    }
                }
                let normals = compute_normals(&positions);
                Cow::Owned(normals)
            }
        };

        create_vertex_buffer(
            "Positions buffer",
            cast_slice(&positions),
            3 * 4,
            wgpu::VertexFormat::Float32x3,
            Attribute::Position,
        );
        create_vertex_buffer(
            "Normals buffer",
            cast_slice(&normals),
            3 * 4,
            wgpu::VertexFormat::Float32x3,
            Attribute::Normal,
        );

        for (attribute, tex_coords) in &self.tex_coords {
            let (contents, array_stride, format) = match tex_coords {
                TexCoords::U8(slice) => (cast_slice(slice), 2 * 1, wgpu::VertexFormat::Unorm8x2),
                TexCoords::U16(slice) => (cast_slice(slice), 2 * 2, wgpu::VertexFormat::Unorm16x2),
                TexCoords::F32(slice) => (cast_slice(slice), 2 * 4, wgpu::VertexFormat::Float32x2),
            };
            create_vertex_buffer(
                "Texture coordinate buffer",
                contents,
                array_stride,
                format,
                *attribute,
            );
        }

        for (attribute, colors) in &self.colors {
            let (contents, array_stride, format) = match colors {
                Colors::RgbaU8(slice) => (cast_slice(slice), 4 * 1, wgpu::VertexFormat::Unorm8x4),
                Colors::RgbaU16(slice) => (cast_slice(slice), 4 * 2, wgpu::VertexFormat::Unorm16x4),
                Colors::RgbaF32(slice) => (cast_slice(slice), 4 * 4, wgpu::VertexFormat::Float32x4),
            };
            create_vertex_buffer(
                "Texture coordinate buffer",
                contents,
                array_stride,
                format,
                *attribute,
            );
        }

        let mut check_attribute =
            |attribute: Attribute, flag: Attributes, dummy_buffer: &DummyVertexBuffer| {
                if !self.attributes.contains(flag) {
                    vertex_buffers.push(dummy_buffer.buffer.clone());
                    vertex_buffer_layouts.push(VertexBufferLayout {
                        array_stride: 0,
                        attributes: vec![wgpu::VertexAttribute {
                            format: dummy_buffer.format,
                            offset: 0,
                            shader_location: attribute as u32,
                        }],
                    });
                }
            };

        check_attribute(
            Attribute::TexCoord0,
            Attributes::TEX_COORD_0,
            &self.resources.geometries().dummy_tex_coords,
        );
        check_attribute(
            Attribute::TexCoord1,
            Attributes::TEX_COORD_1,
            &self.resources.geometries().dummy_tex_coords,
        );
        check_attribute(
            Attribute::Color0,
            Attributes::COLOR_0,
            &self.resources.geometries().dummy_colors,
        );

        let indices = match &self.indices {
            Indices::None => None,
            Indices::U16(slice) => {
                Some((cast_slice(slice), wgpu::IndexFormat::Uint16, slice.len()))
            }
            Indices::U32(slice) => {
                Some((cast_slice(slice), wgpu::IndexFormat::Uint32, slice.len()))
            }
        }
        .map(|(contents, format, index_count)| {
            vertex_count = index_count;
            let buffer =
                self.resources
                    .device()
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Geometry index buffer"),
                        contents,
                        usage: wgpu::BufferUsages::INDEX,
                    });
            IndexBuffer { buffer, format }
        });

        let id = self.resources.geometries().geometries.next_id();
        self.resources.geometries_mut().geometries.insert(Geometry {
            id,
            indices,
            vertex_buffers,
            vertex_buffer_layouts,
            vertex_count: vertex_count as u32,
        })
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

enum TexCoords<'a> {
    U8(Cow<'a, [[u8; 2]]>),
    U16(Cow<'a, [[u16; 2]]>),
    F32(Cow<'a, [[f32; 2]]>),
}

impl<'a> TexCoords<'a> {
    fn to_unindexed(&self, indices: &[usize]) -> Self {
        match self {
            TexCoords::U8(slice) => {
                TexCoords::U8(indices.iter().map(|index| slice[*index]).collect())
            }
            TexCoords::U16(slice) => {
                TexCoords::U16(indices.iter().map(|index| slice[*index]).collect())
            }
            TexCoords::F32(slice) => {
                TexCoords::F32(indices.iter().map(|index| slice[*index]).collect())
            }
        }
    }
}

enum Colors<'a> {
    RgbaU8(Cow<'a, [[u8; 4]]>),
    RgbaU16(Cow<'a, [[u16; 4]]>),
    RgbaF32(Cow<'a, [[f32; 4]]>),
}

impl<'a> Colors<'a> {
    fn to_unindexed(&self, indices: &[usize]) -> Self {
        match self {
            Colors::RgbaU8(slice) => {
                Colors::RgbaU8(indices.iter().map(|index| slice[*index]).collect())
            }
            Colors::RgbaU16(slice) => {
                Colors::RgbaU16(indices.iter().map(|index| slice[*index]).collect())
            }
            Colors::RgbaF32(slice) => {
                Colors::RgbaF32(indices.iter().map(|index| slice[*index]).collect())
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(usize)]
enum Attribute {
    Position = 1,
    Normal = 2,
    TexCoord0 = 4,
    TexCoord1 = 5,
    Color0 = 6,
}

bitflags! {
    struct Attributes: u8 {
        const POSITION = 1 << 0;
        const NORMAL = 1 << 1;
        const TEX_COORD_0 = 1 << 2;
        const TEX_COORD_1 = 1 << 3;
        const COLOR_0 = 1 << 4;
    }
}

struct DummyVertexBuffer {
    buffer: wgpu::Buffer,
    format: wgpu::VertexFormat,
}

fn compute_normals(positions: &[[f32; 3]]) -> Vec<[f32; 3]> {
    let mut normals = Vec::with_capacity(positions.len());
    let mut iter = positions.iter().copied();
    while let Some((a, b, c)) = next_triangle(&mut iter) {
        let ab = b - a;
        let ac = c - a;
        let normal = ab.cross(ac).to_array();
        normals.extend(repeat_n(normal, 3));
    }
    normals
}

fn next_triangle(mut positions: impl Iterator<Item = [f32; 3]>) -> Option<(Vec3, Vec3, Vec3)> {
    Some((
        Vec3::from_array(positions.next()?),
        Vec3::from_array(positions.next()?),
        Vec3::from_array(positions.next()?),
    ))
}
