use std::f32::consts::PI;

use bitflags::bitflags;
use bytemuck::{Pod, Zeroable, bytes_of, cast_slice};
use glam::{UVec4, Vec2, Vec3, Vec4, vec2, vec3, vec4};
use mikktspace_sys::{MikkTSpaceInterface, gen_tang_space_default};

use crate::{DenseEntry, Id, Resources};

pub const MAX_MORPH_TARGET_COUNT: usize = 8;

pub struct Geometry {
    id: Id<Self>,
    indices: Option<Indices>,
    geometry_storage_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    vertex_count: usize,
    morph_target_count: usize,
    attribute_flags: AttributeFlags,
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
        self.morph_target_count
    }

    pub fn has_normal(&self) -> bool {
        self.attribute_flags.contains(AttributeFlags::NORMAL)
    }

    pub fn has_tangents(&self) -> bool {
        self.attribute_flags.contains(AttributeFlags::TANGENT)
    }

    pub fn topology(&self) -> wgpu::PrimitiveTopology {
        self.topology
    }

    pub fn attribute_flags(&self) -> AttributeFlags {
        self.attribute_flags
    }

    pub fn geometry_storage_buffer(&self) -> &wgpu::Buffer {
        &self.geometry_storage_buffer
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
    pub struct AttributeFlags: u8 {
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
pub struct GeometryBuilder {
    vertex_count: usize,
    morph_target_count: usize,
    attributes: Vec<Attribute>,
    attribute_flags: AttributeFlags,
    indices: Option<Indices>,
    normal_tex_coord: Option<u32>,
    topology: wgpu::PrimitiveTopology,
}

impl GeometryBuilder {
    pub fn new(vertex_count: usize, morph_target_count: usize) -> Self {
        let attributes = vec![Attribute::ZERO; (1 + morph_target_count) * vertex_count];

        Self {
            vertex_count,
            morph_target_count,
            attributes,
            attribute_flags: AttributeFlags::empty(),
            indices: None,
            normal_tex_coord: None,
            topology: wgpu::PrimitiveTopology::TriangleList,
        }
    }

    fn indices(
        mut self,
        bytes: &[u8],
        format: wgpu::IndexFormat,
        count: usize,
        resources: &mut Resources,
    ) -> Self {
        let size = padded_size(bytes.len());

        let buffer = resources.device.create_buffer(&wgpu::BufferDescriptor {
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

    pub fn indices_u16(self, indices: &[u16], resources: &mut Resources) -> Self {
        self.indices(
            cast_slice(indices),
            wgpu::IndexFormat::Uint16,
            indices.len(),
            resources,
        )
    }

    pub fn indices_u32(self, indices: &[u32], resources: &mut Resources) -> Self {
        self.indices(
            cast_slice(indices),
            wgpu::IndexFormat::Uint32,
            indices.len(),
            resources,
        )
    }

    fn update_attributes<'a, Values: IntoIterator>(
        mut self,
        mut update: impl FnMut(&mut Attribute, Values::Item),
        values: Values,
        idx: usize,
    ) -> Self {
        let start = idx * self.vertex_count;
        let end = start + self.vertex_count;
        self.attributes[start..end]
            .iter_mut()
            .zip(values.into_iter())
            .for_each(|(attribute, value)| update(attribute, value));
        self
    }

    pub fn positions(mut self, positions: impl IntoIterator<Item = Vec3>) -> Self {
        self.attribute_flags.insert(AttributeFlags::POSITION);
        self.update_attributes(|attr, pos| attr.position = pos, positions, 0)
    }

    pub fn normals(mut self, normals: impl IntoIterator<Item = Vec3>) -> Self {
        self.attribute_flags.insert(AttributeFlags::NORMAL);
        self.update_attributes(|attr, normal| attr.normal = normal, normals, 0)
    }

    pub fn tangents(mut self, tangents: impl IntoIterator<Item = Vec4>) -> Self {
        self.attribute_flags.insert(AttributeFlags::TANGENT);
        self.update_attributes(|attr, tangent| attr.tangent = tangent, tangents, 0)
    }

    pub fn tex_coords_0(mut self, tex_coords_0: impl IntoIterator<Item = Vec2>) -> Self {
        self.attribute_flags.insert(AttributeFlags::TEX_COORD_0);
        self.update_attributes(|attr, tc| attr.tex_coord_0 = tc, tex_coords_0, 0)
    }

    pub fn tex_coords_1(mut self, tex_coords_1: impl IntoIterator<Item = Vec2>) -> Self {
        self.attribute_flags.insert(AttributeFlags::TEX_COORD_1);
        self.update_attributes(|attr, tc| attr.tex_coord_1 = tc, tex_coords_1, 0)
    }

    pub fn colors_0(mut self, colors_0: impl IntoIterator<Item = Vec4>) -> Self {
        self.attribute_flags.insert(AttributeFlags::COLOR_0);
        self.update_attributes(|attr, color| attr.color_0 = color, colors_0, 0)
    }

    pub fn joints_0(mut self, joints_0: impl IntoIterator<Item = UVec4>) -> Self {
        self.attribute_flags.insert(AttributeFlags::JOINTS_0);
        self.update_attributes(|attr, joints| attr.joints_0 = joints, joints_0, 0)
    }

    pub fn weights_0(mut self, weights_0: impl IntoIterator<Item = Vec4>) -> Self {
        self.attribute_flags.insert(AttributeFlags::WEIGHTS_0);
        self.update_attributes(|attr, weights| attr.weights_0 = weights, weights_0, 0)
    }

    pub fn normal_tex_coord(mut self, normal_tex_coord: u32) -> Self {
        self.normal_tex_coord = Some(normal_tex_coord);
        self
    }

    fn update_morph_target_attributes<Values: IntoIterator>(
        self,
        target: usize,
        update: impl FnMut(&mut Attribute, Values::Item),
        values: Values,
    ) -> Self {
        assert!(target < self.morph_target_count);
        self.update_attributes(update, values, 1 + target)
    }

    pub fn morph_target_positions(
        self,
        target: usize,
        positions: impl IntoIterator<Item = Vec3>,
    ) -> Self {
        self.update_morph_target_attributes(target, |attr, pos| attr.position = pos, positions)
    }

    pub fn morph_target_normals(
        self,
        target: usize,
        normals: impl IntoIterator<Item = Vec3>,
    ) -> Self {
        self.update_morph_target_attributes(target, |attr, normal| attr.normal = normal, normals)
    }

    pub fn morph_target_tangents(
        self,
        target: usize,
        tangents: impl IntoIterator<Item = Vec3>,
    ) -> Self {
        self.update_morph_target_attributes(
            target,
            |attr, tangent| attr.tangent = tangent.extend(0.0),
            tangents,
        )
    }

    pub fn morph_target_tex_coords_0(
        self,
        target: usize,
        tex_coords_0: impl IntoIterator<Item = Vec2>,
    ) -> Self {
        self.update_morph_target_attributes(target, |attr, tc| attr.tex_coord_0 = tc, tex_coords_0)
    }

    pub fn morph_target_tex_coords_1(
        self,
        target: usize,
        tex_coords_1: impl IntoIterator<Item = Vec2>,
    ) -> Self {
        self.update_morph_target_attributes(target, |attr, tc| attr.tex_coord_1 = tc, tex_coords_1)
    }

    pub fn morph_target_colors_0(
        self,
        target: usize,
        colors_0: impl IntoIterator<Item = Vec4>,
    ) -> Self {
        self.update_morph_target_attributes(target, |attr, color| attr.color_0 = color, colors_0)
    }

    pub fn morph_target_joints_0(
        self,
        target: usize,
        joints_0: impl IntoIterator<Item = UVec4>,
    ) -> Self {
        self.update_morph_target_attributes(target, |attr, joints| attr.joints_0 = joints, joints_0)
    }

    pub fn morph_target_weights_0(
        self,
        target: usize,
        weights_0: impl IntoIterator<Item = Vec4>,
    ) -> Self {
        self.update_morph_target_attributes(
            target,
            |attr, weights| attr.weights_0 = weights,
            weights_0,
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
    pub fn sphere(self, desc: &SphereDescriptor, resources: &mut Resources) -> Self {
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

        self.indices_u32(&indices, resources)
            .positions(positions)
            .normals(normals)
            .tex_coords_0(uvs)
    }

    pub fn build<'r>(
        mut self,
        resources: &'r mut Resources,
        _encoder: &mut wgpu::CommandEncoder,
    ) -> &'r mut Geometry {
        assert!(
            self.attribute_flags.contains(AttributeFlags::POSITION),
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
                let generate = !self.attribute_flags.contains(AttributeFlags::NORMAL);
                self.attribute_flags.insert(AttributeFlags::NORMAL);
                generate
            }
        };
        if generate_normals {
            // ignore provided tangents
            self.attribute_flags.remove(AttributeFlags::TANGENT);
        } else if self.attribute_flags.contains(AttributeFlags::TANGENT) {
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
                    let mut new_attributes =
                        Vec::with_capacity((1 + self.morph_target_count) * count);

                    let index_view = buffer.slice(..).get_mapped_range();

                    match format {
                        wgpu::IndexFormat::Uint16 => {
                            let indices: &[u16] =
                                cast_slice(&index_view[..count * format.byte_size()]);

                            for target in 0..=self.morph_target_count {
                                let start = target * self.vertex_count;
                                let end = start + self.vertex_count;
                                let old_attr = &self.attributes[start..end];

                                new_attributes
                                    .extend(indices.iter().map(|idx| old_attr[*idx as usize]));
                            }
                        }
                        wgpu::IndexFormat::Uint32 => {
                            let indices: &[u32] = cast_slice(&index_view);

                            for target in 0..=self.morph_target_count {
                                let start = target * self.vertex_count;
                                let end = start + self.vertex_count;
                                let old_attr = &self.attributes[start..end];

                                new_attributes
                                    .extend(indices.iter().map(|idx| old_attr[*idx as usize]));
                            }
                        }
                    }

                    self.attributes = new_attributes;
                    self.vertex_count = count;
                }
            };
        }

        if generate_normals {
            for target in 0..=self.morph_target_count {
                let start = target * self.vertex_count;
                let end = start + self.vertex_count;
                let attributes = &mut self.attributes[start..end];

                compute_normals(attributes);
            }
        }

        if let Some(normal_tex_coord) = self.normal_tex_coord {
            for target in 0..=self.morph_target_count {
                let start = target * self.vertex_count;
                let end = start + self.vertex_count;
                let attributes = &mut self.attributes[start..end];

                let mut mikk_t_space = MikkTSpace {
                    attributes,
                    normal_tex_coord,
                };
                gen_tang_space_default(&mut mikk_t_space);
            }
        }

        let header = GeometryStorageHeader {
            vertex_count: self.vertex_count as u32,
            target_count: self.morph_target_count as u32,
            _pad: [0; 2],
        };

        let attr_start = size_of::<GeometryStorageHeader>();
        let attr_end =
            attr_start + (1 + self.morph_target_count) * self.vertex_count * size_of::<Attribute>();
        let size = padded_size(attr_end);
        let geometry_storage_buffer = resources.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Geometry storage buffer"),
            size,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: true,
        });

        let mut view = geometry_storage_buffer.slice(..).get_mapped_range_mut();
        view[0..attr_start].copy_from_slice(bytes_of(&header));
        view[attr_start..attr_end].copy_from_slice(cast_slice(&self.attributes));
        drop(view);

        geometry_storage_buffer.unmap();
        self.indices.as_ref().map(|indices| indices.buffer.unmap());

        let bind_group = resources
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Geometry bind group"),
                layout: &resources.geometry_builder_data.bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: geometry_storage_buffer.as_entire_binding(),
                }],
            });

        let id = resources.geometries.next_id();
        resources.geometries.insert(Geometry {
            id,
            vertex_count: self.vertex_count,
            morph_target_count: self.morph_target_count,
            bind_group,
            topology: self.topology,
            indices: self.indices,
            geometry_storage_buffer,
            attribute_flags: self.attribute_flags,
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

impl Attribute {
    /// All zeroes.
    const ZERO: Attribute = Attribute {
        position: Vec3::ZERO,
        _pad0: 0,
        tangent: Vec4::ZERO,
        _pad1: 0,
        normal: Vec3::ZERO,
        tex_coord_0: Vec2::ZERO,
        tex_coord_1: Vec2::ZERO,
        color_0: Vec4::ZERO,
        joints_0: UVec4::ZERO,
        weights_0: Vec4::ZERO,
    };
}

fn padded_size(size: usize) -> wgpu::BufferAddress {
    // code taken from wgpu::util::DeviceExt::create_buffer_init()
    let unpadded_size = size as wgpu::BufferAddress;

    let align_mask = wgpu::COPY_BUFFER_ALIGNMENT - 1;

    let padded_size = ((unpadded_size + align_mask) & !align_mask).max(wgpu::COPY_BUFFER_ALIGNMENT);

    padded_size
}

// const POSITION: Range<usize> = 0..12;
// const NORMAL: Range<usize> = 16..28;
// const TANGENT: Range<usize> = 32..48;
// const TEX_COORD_0: Range<usize> = 48..56;
// const TEX_COORD_1: Range<usize> = 56..64;
// const COLOR_0: Range<usize> = 64..80;
// const JOINTS_0: Range<usize> = 80..96;
// const WEIGHTS_0: Range<usize> = 96..112;

// #[cfg(test)]
// mod tests {
//     use glam::uvec4;

//     use super::*;

//     #[test]
//     fn test_attribute_layout() {
//         let attribute = Attribute {
//             position: vec3(0.0, 0.1, 0.2),
//             _pad0: 1,
//             normal: vec3(2.0, 2.1, 2.2),
//             _pad1: 3,
//             tangent: vec4(4.0, 4.1, 4.2, 4.3),
//             tex_coord_0: vec2(5.0, 5.1),
//             tex_coord_1: vec2(6.0, 6.1),
//             color_0: vec4(7.0, 7.1, 7.2, 7.2),
//             joints_0: uvec4(8, 9, 10, 11),
//             weights_0: vec4(13.0, 13.1, 13.2, 13.3),
//         };

//         let bytes = bytes_of(&attribute);

//         assert_eq!(&bytes[POSITION], bytes_of(&attribute.position));
//         assert_eq!(&bytes[NORMAL], bytes_of(&attribute.normal));
//         assert_eq!(&bytes[TANGENT], bytes_of(&attribute.tangent));
//         assert_eq!(&bytes[TEX_COORD_0], bytes_of(&attribute.tex_coord_0));
//         assert_eq!(&bytes[TEX_COORD_1], bytes_of(&attribute.tex_coord_1));
//         assert_eq!(&bytes[COLOR_0], bytes_of(&attribute.color_0));
//         assert_eq!(&bytes[JOINTS_0], bytes_of(&attribute.joints_0));
//         assert_eq!(&bytes[WEIGHTS_0], bytes_of(&attribute.weights_0));
//     }
// }
