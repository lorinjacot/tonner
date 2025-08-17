use std::{f32::consts::PI, iter::zip, ops::Range};

use bitflags::bitflags;
use bytemuck::{Pod, Zeroable, bytes_of, cast_slice, cast_slice_mut};
use glam::{UVec4, Vec2, Vec3, Vec4, vec2, vec3, vec4};
use mikktspace_sys::{MikkTSpaceInterface, gen_tang_space_default};

use crate::{DenseEntry, Id, Resources};

pub const MAX_MORPH_TARGET_COUNT: usize = 8;

pub struct Geometry {
    id: Id<Self>,
    indices: Option<Indices>,
    vertex_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    vertex_count: usize,
    targets: Vec<AttributeFlags>,
    topology: wgpu::PrimitiveTopology,
}

impl Geometry {
    pub(super) fn indices(&self) -> &Option<Indices> {
        &self.indices
    }

    pub fn vertex_count(&self) -> usize {
        self.vertex_count
    }

    pub fn morph_target_count(&self) -> usize {
        self.targets.len() - 1
    }

    pub fn has_normal(&self) -> bool {
        self.targets[0].contains(AttributeFlags::NORMAL)
    }

    pub fn has_tangents(&self) -> bool {
        self.targets[0].contains(AttributeFlags::TANGENT)
    }

    pub fn topology(&self) -> wgpu::PrimitiveTopology {
        self.topology
    }

    pub fn attributes_buffer(&self) -> &wgpu::Buffer {
        &self.vertex_buffer
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}

impl DenseEntry for Geometry {
    type Key = Self;

    fn id(&self) -> Id<Self::Key> {
        self.id
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    struct AttributeFlags: u8 {
        const POSITION    = 1 << 0;
        const NORMAL      = 1 << 1;
        const TANGENT     = 1 << 2;
        const TEX_COORD_0 = 1 << 3;
        const TEX_COORD_1 = 1 << 4;
        const COLOR_0     = 1 << 5;
        const JOINTS_0    = 1 << 6;
        const WEIGHTS_0   = 1 << 7;
    }
}

#[derive(Debug, Clone)]
pub(super) struct Indices {
    pub(super) buffer: wgpu::Buffer,
    pub(super) format: wgpu::IndexFormat,
    pub(super) count: usize,
}

#[must_use]
pub struct GeometryBuilder<'r> {
    resources: &'r mut Resources,
    vertex_count: usize,
    vertex_buffer: wgpu::Buffer,
    targets: Vec<AttributeFlags>,
    indices: Option<Indices>,
    normal_tex_coord: Option<u32>,
    topology: wgpu::PrimitiveTopology,
}

impl<'r> GeometryBuilder<'r> {
    pub fn new(
        vertex_count: usize,
        morph_target_count: usize,
        resources: &'r mut Resources,
    ) -> Self {
        let size = padded_size(
            size_of::<GeometryStorageHeader>()
                + vertex_count * (1 + morph_target_count) * size_of::<Attribute>(),
        );

        let vertex_buffer = resources.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Geometry vertex buffer"),
            size,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: true,
        });

        let targets = vec![AttributeFlags::empty(); 1 + morph_target_count];

        Self {
            resources,
            vertex_count,
            vertex_buffer,
            targets,
            indices: None,
            normal_tex_coord: None,
            topology: wgpu::PrimitiveTopology::TriangleList,
        }
    }

    fn indices(mut self, bytes: &[u8], format: wgpu::IndexFormat, count: usize) -> Self {
        let size = padded_size(bytes.len());

        let buffer = self
            .resources
            .device
            .create_buffer(&wgpu::BufferDescriptor {
                label: Some("Geometry index buffer"),
                size,
                usage: wgpu::BufferUsages::INDEX,
                mapped_at_creation: true,
            });

        buffer.slice(..).get_mapped_range_mut()[0..bytes.len()].copy_from_slice(bytes);

        self.indices = Some(Indices {
            buffer,
            format,
            count,
        });

        self
    }

    pub fn indices_u16(self, indices: &[u16]) -> Self {
        self.indices(cast_slice(indices), wgpu::IndexFormat::Uint16, indices.len())
    }

    pub fn indices_u32(self, indices: &[u32]) -> Self {
        self.indices(cast_slice(indices), wgpu::IndexFormat::Uint32, indices.len())
    }

    fn set_attribute<'a, V, I>(
        mut self,
        flag: AttributeFlags,
        slice: Range<usize>,
        values: V,
        idx: usize,
    ) -> Self
    where
        V: Iterator<Item = I>,
        I: Pod,
    {
        let start = (size_of::<GeometryStorageHeader>()
            + idx * self.vertex_count * size_of::<Attribute>()) as u64;
        let end = start + (self.vertex_count * size_of::<Attribute>()) as u64;

        let mut view = self.vertex_buffer.slice(start..end).get_mapped_range_mut();

        view.chunks_mut(size_of::<Attribute>())
            .zip(values)
            .for_each(|(view, value)| view[slice.clone()].copy_from_slice(bytes_of(&value)));

        drop(view);

        self.targets[idx].insert(flag);
        self
    }

    fn set_attribute_bytes<'a, B>(
        mut self,
        flag: AttributeFlags,
        slice: Range<usize>,
        bytes: B,
        idx: usize,
    ) -> Self
    where
        B: Iterator<Item = &'a [u8]>,
    {
        let start = (size_of::<GeometryStorageHeader>()
            + idx * self.vertex_count * size_of::<Attribute>()) as u64;
        let end = start + (self.vertex_count * size_of::<Attribute>()) as u64;

        let mut view = self.vertex_buffer.slice(start..end).get_mapped_range_mut();

        view.chunks_mut(size_of::<Attribute>())
            .zip(bytes)
            .for_each(|(view, bytes)| view[slice.clone()].copy_from_slice(bytes));

        drop(view);

        self.targets[idx].insert(flag);
        self
    }

    pub fn positions(self, positions: impl IntoIterator<Item = Vec3>) -> Self {
        self.set_attribute(AttributeFlags::POSITION, POSITION, positions.into_iter(), 0)
    }

    pub fn normals(self, normals: impl IntoIterator<Item = Vec3>) -> Self {
        self.set_attribute(AttributeFlags::NORMAL, NORMAL, normals.into_iter(), 0)
    }

    pub fn tangents(self, tangents: impl IntoIterator<Item = Vec4>) -> Self {
        self.set_attribute(AttributeFlags::TANGENT, TANGENT, tangents.into_iter(), 0)
    }

    pub fn tex_coords_0(self, tex_coords_0: impl IntoIterator<Item = Vec2>) -> Self {
        self.set_attribute(
            AttributeFlags::TEX_COORD_0,
            TEX_COORD_0,
            tex_coords_0.into_iter(),
            0,
        )
    }

    pub fn tex_coords_1(self, tex_coords_1: impl IntoIterator<Item = Vec2>) -> Self {
        self.set_attribute(
            AttributeFlags::TEX_COORD_1,
            TEX_COORD_1,
            tex_coords_1.into_iter(),
            0,
        )
    }

    pub fn colors_0(self, colors_0: impl IntoIterator<Item = Vec4>) -> Self {
        self.set_attribute(AttributeFlags::COLOR_0, COLOR_0, colors_0.into_iter(), 0)
    }

    pub fn joints_0(self, joints_0: impl IntoIterator<Item = UVec4>) -> Self {
        self.set_attribute(AttributeFlags::JOINTS_0, JOINTS_0, joints_0.into_iter(), 0)
    }

    pub fn weights_0(self, weights_0: impl IntoIterator<Item = Vec4>) -> Self {
        self.set_attribute(
            AttributeFlags::WEIGHTS_0,
            WEIGHTS_0,
            weights_0.into_iter(),
            0,
        )
    }

    pub fn normal_tex_coord(mut self, normal_tex_coord: u32) -> Self {
        self.normal_tex_coord = Some(normal_tex_coord);
        self
    }

    fn set_morph_target_attribute<'a, V, I>(
        self,
        flag: AttributeFlags,
        slice: Range<usize>,
        values: V,
        target: usize,
    ) -> Self
    where
        V: Iterator<Item = I>,
        I: Pod,
    {
        let idx = target + 1;
        assert!(idx < self.targets.len());
        self.set_attribute(flag, slice, values, idx)
    }

    fn set_morph_target_attribute_bytes<'a, B>(
        self,
        flag: AttributeFlags,
        slice: Range<usize>,
        bytes: B,
        target: usize,
    ) -> Self
    where
        B: Iterator<Item = &'a [u8]>,
    {
        let idx = target + 1;
        assert!(idx < self.targets.len());
        self.set_attribute_bytes(flag, slice, bytes, idx)
    }

    pub fn morph_target_positions(
        self,
        target: usize,
        positions: impl IntoIterator<Item = Vec3>,
    ) -> Self {
        self.set_morph_target_attribute(
            AttributeFlags::POSITION,
            POSITION,
            positions.into_iter(),
            target,
        )
    }

    pub fn morph_target_normals(
        self,
        target: usize,
        normals: impl IntoIterator<Item = Vec3>,
    ) -> Self {
        self.set_morph_target_attribute(AttributeFlags::NORMAL, NORMAL, normals.into_iter(), target)
    }

    pub fn morph_target_tangents(
        self,
        target: usize,
        tangents: impl IntoIterator<Item = Vec3>,
    ) -> Self {
        self.set_morph_target_attribute(
            AttributeFlags::TANGENT,
            TANGENT,
            tangents.into_iter(),
            target,
        )
    }

    pub fn morph_target_tex_coords_0(
        self,
        target: usize,
        tex_coords_0: impl IntoIterator<Item = Vec2>,
    ) -> Self {
        self.set_morph_target_attribute(
            AttributeFlags::TEX_COORD_0,
            TEX_COORD_0,
            tex_coords_0.into_iter(),
            target,
        )
    }

    pub fn morph_target_tex_coords_1(
        self,
        target: usize,
        tex_coords_1: impl IntoIterator<Item = Vec2>,
    ) -> Self {
        self.set_morph_target_attribute(
            AttributeFlags::TEX_COORD_1,
            TEX_COORD_1,
            tex_coords_1.into_iter(),
            target,
        )
    }

    pub fn morph_target_colors_0(
        self,
        target: usize,
        colors_0: impl IntoIterator<Item = Vec4>,
    ) -> Self {
        self.set_morph_target_attribute(
            AttributeFlags::COLOR_0,
            COLOR_0,
            colors_0.into_iter(),
            target,
        )
    }

    pub fn morph_target_joints_0(
        self,
        target: usize,
        joints_0: impl IntoIterator<Item = UVec4>,
    ) -> Self {
        self.set_morph_target_attribute(
            AttributeFlags::JOINTS_0,
            JOINTS_0,
            joints_0.into_iter(),
            target,
        )
    }

    pub fn morph_target_weights_0(
        self,
        target: usize,
        weights_0: impl IntoIterator<Item = Vec4>,
    ) -> Self {
        self.set_morph_target_attribute(
            AttributeFlags::WEIGHTS_0,
            WEIGHTS_0,
            weights_0.into_iter(),
            target,
        )
    }

    pub fn topology(mut self, topology: wgpu::PrimitiveTopology) -> Self {
        self.topology = topology;
        self
    }

    /// An helper for generating sphere geometries.
    /// Based on [three.js/SphereGeometry](https://threejs.org/docs/#api/en/geometries/SphereGeometry).
    ///
    /// The geometry is created by sweeping and calculating vertexes around the Y axis (horizontal sweep)
    /// and the Z axis (vertical sweep). Thus, incomplete spheres (akin to 'sphere slices') can be created
    /// through the use of different values of `phi_start`, `phi_length`, `theta_start` and `theta_length`,
    /// in order to define the points in which we start (or end) calculating those vertices.
    pub fn sphere(self, desc: &SphereDescriptor) -> Self {
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

                positions.push(vertex);
                normals.push(vertex.normalize());
                uvs.push(vec2(u + u_offset, 1.0 - v));

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

        self.indices_u32(&indices)
            .positions(positions)
            .normals(normals)
            .tex_coords_0(uvs)
    }

    pub fn build(mut self, _encoder: &mut wgpu::CommandEncoder) -> &'r mut Geometry {
        assert!(
            self.targets[0].contains(AttributeFlags::POSITION),
            "position attribute should be set"
        );

        let generate_normals = match self.topology {
            wgpu::PrimitiveTopology::PointList
            | wgpu::PrimitiveTopology::LineList
            | wgpu::PrimitiveTopology::LineStrip => {
                self.normal_tex_coord = None;
                false
            }
            _ => {
                let generate = !self.targets[0].contains(AttributeFlags::NORMAL);
                self.targets[0].insert(AttributeFlags::NORMAL);
                generate
            }
        };
        if generate_normals {
            // ignore provided tangents
            self.targets[0].remove(AttributeFlags::TANGENT);
        } else if self.targets[0].contains(AttributeFlags::TANGENT) {
            // use provided tangents
            self.normal_tex_coord = None;
        }

        // we cannot generate normals and tangents with indexed geometries
        if generate_normals || self.normal_tex_coord.is_some() {
            match self.indices.take() {
                None => (),
                Some(Indices {
                    buffer,
                    format,
                    count,
                }) => {
                    let size = padded_size(
                        size_of::<GeometryStorageHeader>()
                            + self.targets.len() * count * size_of::<Attribute>(),
                    );

                    let vertex_buffer =
                        self.resources
                            .device
                            .create_buffer(&wgpu::BufferDescriptor {
                                label: Some("Geometry vertex buffer"),
                                size,
                                usage: wgpu::BufferUsages::STORAGE,
                                mapped_at_creation: true,
                            });

                    let index_view = buffer.slice(..).get_mapped_range();

                    match format {
                        wgpu::IndexFormat::Uint16 => {
                            let indices: &[u16] = cast_slice(&index_view);

                            for idx in 0..self.targets.len() {
                                let start = size_of::<GeometryStorageHeader>()
                                    + idx * self.vertex_count * size_of::<Attribute>();
                                let end = start + self.vertex_count * size_of::<Attribute>();
                                let old_view = self
                                    .vertex_buffer
                                    .slice(start as u64..end as u64)
                                    .get_mapped_range();
                                let old_attributes: &[Attribute] = cast_slice(&old_view);

                                let start = size_of::<GeometryStorageHeader>()
                                    + idx * count * size_of::<Attribute>();
                                let end = start + count * size_of::<Attribute>();
                                let mut new_view = vertex_buffer
                                    .slice(start as u64..end as u64)
                                    .get_mapped_range_mut();
                                let new_attributes: &mut [Attribute] =
                                    cast_slice_mut(&mut new_view);

                                assert_eq!(indices.len(), new_attributes.len());
                                zip(indices, new_attributes).for_each(|(idx, attr)| {
                                    *attr = old_attributes[*idx as usize];
                                });
                            }
                        }
                        wgpu::IndexFormat::Uint32 => {
                            let indices: &[u32] = cast_slice(&index_view);

                            for idx in 0..self.targets.len() {
                                let start = size_of::<GeometryStorageHeader>()
                                    + idx * self.vertex_count * size_of::<Attribute>();
                                let end = start + self.vertex_count * size_of::<Attribute>();
                                let old_view = self
                                    .vertex_buffer
                                    .slice(start as u64..end as u64)
                                    .get_mapped_range();
                                let old_attributes: &[Attribute] = cast_slice(&old_view);

                                let start = size_of::<GeometryStorageHeader>()
                                    + idx * count * size_of::<Attribute>();
                                let end = start + count * size_of::<Attribute>();
                                let mut new_view = vertex_buffer
                                    .slice(start as u64..end as u64)
                                    .get_mapped_range_mut();
                                let new_attributes: &mut [Attribute] =
                                    cast_slice_mut(&mut new_view);

                                assert_eq!(indices.len(), new_attributes.len());
                                zip(indices, new_attributes).for_each(|(idx, attr)| {
                                    *attr = old_attributes[*idx as usize];
                                });
                            }
                        }
                    }

                    self.vertex_buffer = vertex_buffer;
                    self.vertex_count = count;
                }
            };
        }

        if generate_normals {
            for target in 0..self.targets.len() {
                let start = size_of::<GeometryStorageHeader>()
                    + target * self.vertex_count * size_of::<Attribute>();
                let end = start + self.vertex_count * size_of::<Attribute>();
                let mut view = self
                    .vertex_buffer
                    .slice(start as u64..end as u64)
                    .get_mapped_range_mut();
                let attributes = cast_slice_mut(&mut view);

                compute_normals(attributes);
            }
        }

        if let Some(normal_tex_coord) = self.normal_tex_coord {
            for target in 0..self.targets.len() {
                let start = size_of::<GeometryStorageHeader>()
                    + target * self.vertex_count * size_of::<Attribute>();
                let end = start + self.vertex_count * size_of::<Attribute>();
                let mut view = self
                    .vertex_buffer
                    .slice(start as u64..end as u64)
                    .get_mapped_range_mut();
                let attributes = cast_slice_mut(&mut view);

                let mut mikk_t_space = MikkTSpace {
                    attributes,
                    normal_tex_coord,
                };
                gen_tang_space_default(&mut mikk_t_space);
            }
        }

        let header = GeometryStorageHeader {
            vertex_count: self.vertex_count as u32,
            target_count: self.targets.len() as u32 - 1,
            _pad: [0; 2],
        };
        let mut view = self
            .vertex_buffer
            .slice(0..size_of::<GeometryStorageHeader>() as u64)
            .get_mapped_range_mut();
        view.copy_from_slice(bytes_of(&header));
        drop(view);

        self.vertex_buffer.unmap();
        self.indices.as_ref().map(|indices| indices.buffer.unmap());

        let bind_group = self
            .resources
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Geometry bind group"),
                layout: &self.resources.geometry_builder_data.bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.vertex_buffer.as_entire_binding(),
                }],
            });

        let id = self.resources.geometries.next_id();
        self.resources.geometries.insert(Geometry {
            id,
            indices: self.indices,
            vertex_buffer: self.vertex_buffer,
            bind_group,
            vertex_count: self.vertex_count,
            targets: self.targets,
            topology: self.topology,
        })
    }
}

pub(super) struct GeometryBuilderData {
    bind_group_layout: wgpu::BindGroupLayout,
}

impl GeometryBuilderData {
    pub fn new(device: &wgpu::Device) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Geometry bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        Self { bind_group_layout }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
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

fn compute_normals(attributes: &mut [Attribute]) {
    let mut iter = attributes.iter_mut();
    while let Some((a, b, c)) = next_triangle(&mut iter) {
        let ab = b.position - a.position;
        let ac = c.position - a.position;
        let normal = ab.cross(ac);
        a.normal = normal;
        b.normal = normal;
        c.normal = normal;
    }
}

fn next_triangle<'a>(
    mut attributes: impl Iterator<Item = &'a mut Attribute>,
) -> Option<(&'a mut Attribute, &'a mut Attribute, &'a mut Attribute)> {
    Some((attributes.next()?, attributes.next()?, attributes.next()?))
}

struct MikkTSpace<'a> {
    attributes: &'a mut [Attribute],
    normal_tex_coord: u32,
}

impl<'a> MikkTSpace<'a> {
    fn attribute(&self, face: usize, vert: usize) -> &Attribute {
        &self.attributes[face * 3 + vert]
    }

    fn attribute_mut(&mut self, face: usize, vert: usize) -> &mut Attribute {
        &mut self.attributes[face * 3 + vert]
    }
}

impl<'a> MikkTSpaceInterface for MikkTSpace<'a> {
    fn get_num_faces(&self) -> usize {
        self.attributes.len() / 3
    }

    fn get_num_vertices_of_face(&self, _face: usize) -> usize {
        3
    }

    fn get_position(&self, face: usize, vert: usize) -> [f32; 3] {
        self.attribute(face, vert).position.to_array()
    }

    fn get_normal(&self, face: usize, vert: usize) -> [f32; 3] {
        self.attribute(face, vert).normal.to_array()
    }

    fn get_tex_coord(&self, face: usize, vert: usize) -> [f32; 2] {
        match self.normal_tex_coord {
            0 => self.attribute(face, vert).tex_coord_0.to_array(),
            1 => self.attribute(face, vert).tex_coord_1.to_array(),
            _ => unreachable!(),
        }
    }

    fn set_tspace_basic(&mut self, tangent: [f32; 3], sign: f32, face: usize, vert: usize) {
        self.attribute_mut(face, vert).tangent = vec4(tangent[0], tangent[1], tangent[2], -sign);
    }
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct GeometryStorageHeader {
    vertex_count: u32,
    target_count: u32,
    _pad: [u32; 2],
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct Attribute {
    position: Vec3,
    _pad0: u32,
    normal: Vec3,
    _pad1: u32,
    tangent: Vec4,
    tex_coord_0: Vec2,
    tex_coord_1: Vec2,
    color_0: Vec4,
    joints_0: UVec4,
    weights_0: Vec4,
}

const POSITION: Range<usize> = 0..12;
const NORMAL: Range<usize> = 16..28;
const TANGENT: Range<usize> = 32..48;
const TEX_COORD_0: Range<usize> = 48..56;
const TEX_COORD_1: Range<usize> = 56..64;
const COLOR_0: Range<usize> = 64..80;
const JOINTS_0: Range<usize> = 80..96;
const WEIGHTS_0: Range<usize> = 96..112;

#[cfg(test)]
mod tests {
    use glam::uvec4;

    use super::*;

    #[test]
    fn test_attribute_layout() {
        let attribute = Attribute {
            position: vec3(0.0, 0.1, 0.2),
            _pad0: 1,
            normal: vec3(2.0, 2.1, 2.2),
            _pad1: 3,
            tangent: vec4(4.0, 4.1, 4.2, 4.3),
            tex_coord_0: vec2(5.0, 5.1),
            tex_coord_1: vec2(6.0, 6.1),
            color_0: vec4(7.0, 7.1, 7.2, 7.2),
            joints_0: uvec4(8, 9, 10, 11),
            weights_0: vec4(13.0, 13.1, 13.2, 13.3),
        };

        let bytes = bytes_of(&attribute);

        assert_eq!(&bytes[POSITION], bytes_of(&attribute.position));
        assert_eq!(&bytes[NORMAL], bytes_of(&attribute.normal));
        assert_eq!(&bytes[TANGENT], bytes_of(&attribute.tangent));
        assert_eq!(&bytes[TEX_COORD_0], bytes_of(&attribute.tex_coord_0));
        assert_eq!(&bytes[TEX_COORD_1], bytes_of(&attribute.tex_coord_1));
        assert_eq!(&bytes[COLOR_0], bytes_of(&attribute.color_0));
        assert_eq!(&bytes[JOINTS_0], bytes_of(&attribute.joints_0));
        assert_eq!(&bytes[WEIGHTS_0], bytes_of(&attribute.weights_0));
    }
}

fn padded_size(size: usize) -> wgpu::BufferAddress {
    // code taken from wgpu::util::DeviceExt::create_buffer_init()
    let unpadded_size = size as wgpu::BufferAddress;

    let align_mask = wgpu::COPY_BUFFER_ALIGNMENT - 1;

    let padded_size = ((unpadded_size + align_mask) & !align_mask).max(wgpu::COPY_BUFFER_ALIGNMENT);

    padded_size
}
