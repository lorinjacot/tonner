use std::{
    borrow::Cow,
    f32::consts::PI,
    iter::{from_fn, repeat},
};

use bytemuck::{Pod, Zeroable, bytes_of, cast_slice};
use glam::{UVec4, Vec2, Vec3, Vec4, vec2, vec3, vec4};
use mikktspace_sys::{MikkTSpaceInterface, gen_tang_space_default};
use wgpu::util::DeviceExt;

use crate::{DenseEntry, Id, Resources};

pub const MAX_MORPH_TARGET_COUNT: usize = 8;

pub struct Geometry {
    id: Id<Self>,
    indices: Indices,
    attributes_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    vertex_count: usize,
    morph_target_count: usize,
    has_normal: bool,
    has_tangents: bool,
    topology: wgpu::PrimitiveTopology,
}

impl Geometry {
    pub(super) fn indices(&self) -> &Indices {
        &self.indices
    }

    pub fn vertex_count(&self) -> usize {
        self.vertex_count
    }

    pub fn morph_target_count(&self) -> usize {
        self.morph_target_count
    }

    pub fn has_normal(&self) -> bool {
        self.has_normal
    }

    pub fn has_tangents(&self) -> bool {
        self.has_tangents
    }

    pub fn topology(&self) -> wgpu::PrimitiveTopology {
        self.topology
    }

    pub fn attributes_buffer(&self) -> &wgpu::Buffer {
        &self.attributes_buffer
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

#[derive(Debug, Clone)]
pub(super) enum Indices {
    Some {
        buffer: wgpu::Buffer,
        format: wgpu::IndexFormat,
        index_count: u32,
    },
    None {
        vertex_count: u32,
    },
}

#[must_use]
pub struct GeometryBuilder<'a, 'r> {
    resources: &'r mut Resources,
    indices: IndicesSlices<'a>,
    attributes: MorphTargetBuilder<'a>,
    normal_tex_coord: Option<u32>,
    targets: Vec<MorphTargetBuilder<'a>>,
    topology: wgpu::PrimitiveTopology,
}

impl<'a, 'r> GeometryBuilder<'a, 'r> {
    pub fn new(resources: &'r mut Resources) -> Self {
        Self {
            resources,
            indices: IndicesSlices::None,
            attributes: MorphTargetBuilder {
                positions: None,
                normals: None,
                tangents: None,
                tex_coords_0: None,
                tex_coords_1: None,
                colors_0: None,
                joints_0: None,
                weights_0: None,
            },
            normal_tex_coord: None,
            targets: Vec::new(),
            topology: wgpu::PrimitiveTopology::TriangleList,
        }
    }

    pub fn indices_u16(mut self, indices: Cow<'a, [u16]>) -> Self {
        self.indices = IndicesSlices::U16(indices);
        self
    }

    pub fn indices_u32(mut self, indices: Cow<'a, [u32]>) -> Self {
        self.indices = IndicesSlices::U32(indices);
        self
    }

    pub fn positions(mut self, positions: impl IntoIterator<Item = Vec3> + 'a) -> Self {
        self.attributes = self.attributes.positions(positions);
        self
    }

    pub fn normals(mut self, normals: impl IntoIterator<Item = Vec3> + 'a) -> Self {
        self.attributes = self.attributes.normals(normals);
        self
    }

    pub fn tangents(mut self, tangents: impl IntoIterator<Item = Vec4> + 'a) -> Self {
        self.attributes.tangents = Some(Box::new(tangents.into_iter()));
        self
    }

    pub fn normal_tex_coord(mut self, normal_tex_coord: u32) -> Self {
        self.normal_tex_coord = Some(normal_tex_coord);
        self
    }

    pub fn tex_coords(mut self, tex_coords: impl IntoIterator<Item = Vec2> + 'a) -> Self {
        self.attributes = self.attributes.tex_coords(tex_coords);
        self
    }

    pub fn colors(mut self, colors: impl IntoIterator<Item = Vec4> + 'a) -> Self {
        self.attributes = self.attributes.colors(colors);
        self
    }

    pub fn joints(mut self, joints: impl IntoIterator<Item = UVec4> + 'a) -> Self {
        self.attributes = self.attributes.joints(joints);
        self
    }

    pub fn weights(mut self, weights: impl IntoIterator<Item = Vec4> + 'a) -> Self {
        self.attributes = self.attributes.weights(weights);
        self
    }

    pub fn morph_target(mut self, morph_target: MorphTargetBuilder<'a>) -> Self {
        self.targets.push(morph_target);
        self
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

        self.indices_u32(indices.into())
            .positions(positions)
            .normals(normals)
            .tex_coords(uvs)
    }

    pub fn build(mut self, _encoder: &mut wgpu::CommandEncoder) -> &'r mut Geometry {
        let (has_normal, generate_normals) = match self.topology {
            wgpu::PrimitiveTopology::PointList
            | wgpu::PrimitiveTopology::LineList
            | wgpu::PrimitiveTopology::LineStrip => {
                self.normal_tex_coord = None;
                (self.attributes.normals.is_some(), false)
            }
            _ => (true, self.attributes.normals.is_none()),
        };
        if generate_normals {
            // ignore provided tangents
            self.attributes.tangents = None;
        }
        let has_tangents = if self.attributes.tangents.is_some() {
            // use provided tangents
            self.normal_tex_coord = None;
            true
        } else {
            self.normal_tex_coord.is_some()
        };

        let mut positions = self
            .attributes
            .positions
            .expect("positions attribute should be set");
        let mut normals = self
            .attributes
            .normals
            .unwrap_or_else(|| Box::new(repeat(Vec3::ZERO)));
        let mut tangents = self
            .attributes
            .tangents
            .unwrap_or_else(|| Box::new(repeat(Vec4::ZERO)));
        let mut tex_coords_0 = self
            .attributes
            .tex_coords_0
            .unwrap_or_else(|| Box::new(repeat(Vec2::ZERO)));
        let mut tex_coords_1 = self
            .attributes
            .tex_coords_1
            .unwrap_or_else(|| Box::new(repeat(Vec2::ZERO)));
        let mut colors_0 = self
            .attributes
            .colors_0
            .unwrap_or_else(|| Box::new(repeat(Vec4::ONE)));
        let mut joints_0 = self
            .attributes
            .joints_0
            .unwrap_or_else(|| Box::new(repeat(UVec4::ZERO)));
        let mut weights_0 = self
            .attributes
            .weights_0
            .unwrap_or_else(|| Box::new(repeat(Vec4::ZERO)));

        let mut vertex_count = positions.size_hint().0;
        let morph_target_count = self.targets.len();
        let mut attributes = Vec::with_capacity(vertex_count * (1 + morph_target_count));

        attributes.extend(from_fn(|| {
            Some(Attribute {
                position: positions.next()?,
                _pad0: 0,
                normal: normals.next()?,
                _pad1: 0,
                tangent: tangents.next()?,
                tex_coord_0: tex_coords_0.next()?,
                tex_coord_1: tex_coords_1.next()?,
                color_0: colors_0.next()?,
                joints_0: joints_0.next()?,
                weights_0: weights_0.next()?,
            })
        }));
        vertex_count = attributes.len();
        assert!(
            self.targets.len() < MAX_MORPH_TARGET_COUNT,
            "Too many morph target"
        );
        for target in self.targets {
            let mut positions = target
                .positions
                .unwrap_or_else(|| Box::new(repeat(Vec3::ZERO)));
            let mut normals = target
                .normals
                .unwrap_or_else(|| Box::new(repeat(Vec3::ZERO)));
            let mut tangents = target
                .tangents
                .unwrap_or_else(|| Box::new(repeat(Vec4::ZERO)));
            let mut tex_coords_0 = target
                .tex_coords_0
                .unwrap_or_else(|| Box::new(repeat(Vec2::ZERO)));
            let mut tex_coords_1 = target
                .tex_coords_1
                .unwrap_or_else(|| Box::new(repeat(Vec2::ZERO)));
            let mut colors_0 = target
                .colors_0
                .unwrap_or_else(|| Box::new(repeat(Vec4::ZERO)));

            attributes.extend(from_fn(|| {
                Some(Attribute {
                    position: positions.next()?,
                    _pad0: 0,
                    normal: normals.next()?,
                    _pad1: 0,
                    tangent: tangents.next()?,
                    tex_coord_0: tex_coords_0.next()?,
                    tex_coord_1: tex_coords_1.next()?,
                    color_0: colors_0.next()?,
                    joints_0: joints_0.next()?,
                    weights_0: weights_0.next()?,
                })
            }));
        }

        // we cannot generate normals and tangents with indexed geometries
        if generate_normals || self.normal_tex_coord.is_some() {
            match self.indices {
                IndicesSlices::None => (),
                IndicesSlices::U16(slice) => {
                    vertex_count = slice.len();
                    let mut new_attributes =
                        Vec::with_capacity((1 + morph_target_count) * vertex_count);
                    for i in 0..=morph_target_count {
                        let offset = i * vertex_count;
                        new_attributes.extend(
                            slice
                                .iter()
                                .map(|index| attributes[offset + *index as usize]),
                        );
                    }
                    attributes = new_attributes;
                }
                IndicesSlices::U32(slice) => {
                    vertex_count = slice.len();
                    let mut new_attributes =
                        Vec::with_capacity((1 + morph_target_count) * vertex_count);
                    for i in 0..=morph_target_count {
                        let offset = i * vertex_count;
                        new_attributes.extend(
                            slice
                                .iter()
                                .map(|index| attributes[offset + *index as usize]),
                        );
                    }
                    attributes = new_attributes;
                }
            };
            self.indices = IndicesSlices::None;
        }

        if generate_normals {
            compute_normals(&mut attributes);
        }

        if let Some(normal_tex_coord) = self.normal_tex_coord {
            let mut mikk_t_space = MikkTSpace {
                attributes: &mut attributes,
                normal_tex_coord,
            };
            gen_tang_space_default(&mut mikk_t_space);
        }

        let header = AttributeStorageHeader {
            vertex_count: vertex_count as u32,
            target_count: morph_target_count as u32,
            _pad: [0; 2],
        };
        let header_size = size_of::<AttributeStorageHeader>() as wgpu::BufferAddress;
        let attributes_size = (vertex_count * (1 + morph_target_count) * size_of::<Attribute>())
            as wgpu::BufferAddress;

        let attributes_buffer = self
            .resources
            .device
            .create_buffer(&wgpu::BufferDescriptor {
                label: Some("Geometry storage buffer"),
                size: header_size + attributes_size,
                usage: wgpu::BufferUsages::STORAGE,
                mapped_at_creation: true,
            });

        attributes_buffer
            .slice(..header_size)
            .get_mapped_range_mut()
            .copy_from_slice(bytes_of(&header));

        attributes_buffer
            .slice(header_size..)
            .get_mapped_range_mut()
            .copy_from_slice(cast_slice(&attributes));

        attributes_buffer.unmap();

        let indices = match &self.indices {
            IndicesSlices::None => None,
            IndicesSlices::U16(slice) => Some((
                cast_slice(slice),
                wgpu::IndexFormat::Uint16,
                slice.len() as u32,
            )),
            IndicesSlices::U32(slice) => Some((
                cast_slice(slice),
                wgpu::IndexFormat::Uint32,
                slice.len() as u32,
            )),
        };
        let indices = match indices {
            Some((contents, format, index_count)) => {
                let buffer =
                    self.resources
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Geometry index buffer"),
                            contents,
                            usage: wgpu::BufferUsages::INDEX,
                        });
                Indices::Some {
                    buffer,
                    format,
                    index_count,
                }
            }
            None => Indices::None {
                vertex_count: vertex_count as u32,
            },
        };

        let bind_group = self
            .resources
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Geometry bind group"),
                layout: &self.resources.geometry_builder_data.bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: attributes_buffer.as_entire_binding(),
                }],
            });

        let id = self.resources.geometries.next_id();
        self.resources.geometries.insert(Geometry {
            id,
            indices,
            attributes_buffer,
            bind_group,
            vertex_count,
            morph_target_count,
            has_normal,
            has_tangents,
            topology: self.topology,
        })
    }
}

#[must_use]
pub struct MorphTargetBuilder<'a> {
    positions: Option<Box<dyn Iterator<Item = Vec3> + 'a>>,
    normals: Option<Box<dyn Iterator<Item = Vec3> + 'a>>,
    tangents: Option<Box<dyn Iterator<Item = Vec4> + 'a>>,
    tex_coords_0: Option<Box<dyn Iterator<Item = Vec2> + 'a>>,
    tex_coords_1: Option<Box<dyn Iterator<Item = Vec2> + 'a>>,
    colors_0: Option<Box<dyn Iterator<Item = Vec4> + 'a>>,
    joints_0: Option<Box<dyn Iterator<Item = UVec4> + 'a>>,
    weights_0: Option<Box<dyn Iterator<Item = Vec4> + 'a>>,
}

impl<'a> MorphTargetBuilder<'a> {
    pub fn new() -> Self {
        Self {
            positions: None,
            normals: None,
            tangents: None,
            tex_coords_0: None,
            tex_coords_1: None,
            colors_0: None,
            joints_0: None,
            weights_0: None,
        }
    }

    pub fn positions(mut self, positions: impl IntoIterator<Item = Vec3> + 'a) -> Self {
        self.positions = Some(Box::new(positions.into_iter()));
        self
    }

    pub fn normals(mut self, normals: impl IntoIterator<Item = Vec3> + 'a) -> Self {
        self.normals = Some(Box::new(normals.into_iter()));
        self
    }

    pub fn tangents(mut self, tangents: impl IntoIterator<Item = Vec3> + 'a) -> Self {
        self.tangents = Some(Box::new(tangents.into_iter().map(|v| v.extend(0.0))));
        self
    }

    pub fn tex_coords(mut self, tex_coords: impl IntoIterator<Item = Vec2> + 'a) -> Self {
        if self.tex_coords_0.is_none() {
            self.tex_coords_0 = Some(Box::new(tex_coords.into_iter()));
        } else if self.tex_coords_1.is_none() {
            self.tex_coords_1 = Some(Box::new(tex_coords.into_iter()));
        } else {
            panic!("to many geometry texture coordinates set");
        }
        self
    }

    pub fn colors(mut self, colors: impl IntoIterator<Item = Vec4> + 'a) -> Self {
        if self.colors_0.is_none() {
            self.colors_0 = Some(Box::new(colors.into_iter()));
        } else {
            panic!("too many geometry colors set");
        }
        self
    }

    pub fn joints(mut self, joints: impl IntoIterator<Item = UVec4> + 'a) -> Self {
        if self.joints_0.is_none() {
            self.joints_0 = Some(Box::new(joints.into_iter()));
        } else {
            panic!("too many geometry joints set");
        }
        self
    }

    pub fn weights(mut self, weights: impl IntoIterator<Item = Vec4> + 'a) -> Self {
        if self.weights_0.is_none() {
            self.weights_0 = Some(Box::new(weights.into_iter()));
        } else {
            panic!("too many geometry weights set");
        }
        self
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

enum IndicesSlices<'a> {
    None,
    U16(Cow<'a, [u16]>),
    U32(Cow<'a, [u32]>),
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
struct AttributeStorageHeader {
    vertex_count: u32,
    target_count: u32,
    _pad: [u32; 2],
}

#[derive(Clone, Copy, Pod, Zeroable)]
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
