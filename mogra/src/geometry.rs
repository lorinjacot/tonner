use bitflags::bitflags;
use bytemuck::{Pod, Zeroable, bytes_of, cast_slice};
use glam::{Quat, UVec4, Vec2, Vec3, Vec4, Vec4Swizzles, vec4};
use thiserror::Error;
use wgpu::util::DeviceExt;

use crate::Context;

pub use arrow::{ArrowBuilder, ArrowParts};
pub use r#box::BoxBuilder;
pub use cone::ConeBuilder;
pub use cylinder::CylinderBuilder;
pub use sphere::{NotEnoughSegmentsError, SphereBuilder};

mod arrow;
mod r#box;
mod cone;
mod cylinder;
mod sphere;

/// The maximum number of morph targets a geometry can have. This is a limitation of the current implementation and may be increased in the future.
pub const MAX_MORPH_TARGET_COUNT: usize = 8;

#[derive(Debug, Clone)]
pub struct Geometry {
    attributes: GeometryAttributes,
    indices: Option<GeometryIndices>,
    topology: wgpu::PrimitiveTopology,
}

#[must_use]
#[derive(Debug, Clone)]
pub struct GeometryBuilder {
    vertex_count: usize,
    morph_target_count: usize,
    attributes: Vec<Attribute>,
    attribute_flags: GeometryFlags,
    indices: Option<Indices>,
    normal_tex_coord: Option<u32>,
    topology: wgpu::PrimitiveTopology,
}

impl GeometryBuilder {
    /// Create a new geometry builder.
    pub fn new(vertex_count: usize, morph_target_count: usize) -> Self {
        let attributes = vec![Attribute::ZERO; (1 + morph_target_count) * vertex_count];

        Self {
            vertex_count,
            morph_target_count,
            attributes,
            attribute_flags: GeometryFlags::empty(),
            indices: None,
            normal_tex_coord: None,
            topology: wgpu::PrimitiveTopology::TriangleList,
        }
    }

    /// Mark the geometry as an indexed geometry and set the indices.
    pub fn indices_u16(mut self, indices: impl IntoIterator<Item = u16>) -> Self {
        self.indices = Some(Indices::U16(indices.into_iter().collect()));
        self
    }

    /// Mark the geometry as an indexed geometry and set the indices.
    pub fn indices_u32(mut self, indices: impl IntoIterator<Item = u32>) -> Self {
        self.indices = Some(Indices::U32(indices.into_iter().collect()));
        self
    }

    fn update_attributes<'a, Values: IntoIterator>(
        mut self,
        mut update: impl FnMut(&mut Attribute, Values::Item),
        values: Values,
        idx: usize,
    ) -> Result<Self, AttributeError> {
        let mut iter = values.into_iter();

        let start = idx * self.vertex_count;
        let end = start + self.vertex_count;

        for attribute in &mut self.attributes[start..end] {
            let value = iter.next().ok_or(AttributeError::InvalidIterator {
                expected_length: self.vertex_count,
            })?;
            update(attribute, value);
        }

        Ok(self)
    }

    /// Set/update the vertices positions.
    pub fn positions(
        mut self,
        positions: impl IntoIterator<Item = Vec3>,
    ) -> Result<Self, AttributeError> {
        self.attribute_flags.insert(GeometryFlags::POSITION);
        self.update_attributes(|attr, pos| attr.position = pos, positions, 0)
    }

    /// Set/update the vertices normals.
    pub fn normals(
        mut self,
        normals: impl IntoIterator<Item = Vec3>,
    ) -> Result<Self, AttributeError> {
        self.attribute_flags.insert(GeometryFlags::NORMAL);
        self.update_attributes(|attr, normal| attr.normal = normal, normals, 0)
    }

    /// Set/update the vertices tangents. Use for normal mapping.
    pub fn tangents(
        mut self,
        tangents: impl IntoIterator<Item = Vec4>,
    ) -> Result<Self, AttributeError> {
        self.attribute_flags.insert(GeometryFlags::TANGENT);
        self.update_attributes(|attr, tangent| attr.tangent = tangent, tangents, 0)
    }

    /// Set/update the vertices first texture coordinates.
    pub fn tex_coords_0(
        mut self,
        tex_coords_0: impl IntoIterator<Item = Vec2>,
    ) -> Result<Self, AttributeError> {
        self.attribute_flags.insert(GeometryFlags::TEX_COORD_0);
        self.update_attributes(|attr, tc| attr.tex_coord_0 = tc, tex_coords_0, 0)
    }

    /// Set/update the vertices second texture coordinates.
    pub fn tex_coords_1(
        mut self,
        tex_coords_1: impl IntoIterator<Item = Vec2>,
    ) -> Result<Self, AttributeError> {
        self.attribute_flags.insert(GeometryFlags::TEX_COORD_1);
        self.update_attributes(|attr, tc| attr.tex_coord_1 = tc, tex_coords_1, 0)
    }

    /// Set/update the vertices color.
    pub fn colors_0(
        mut self,
        colors_0: impl IntoIterator<Item = Vec4>,
    ) -> Result<Self, AttributeError> {
        self.attribute_flags.insert(GeometryFlags::COLOR_0);
        self.update_attributes(|attr, color| attr.color_0 = color, colors_0, 0)
    }

    /// Set/update the vertices joints. Used for skinning.
    pub fn joints_0(
        mut self,
        joints_0: impl IntoIterator<Item = UVec4>,
    ) -> Result<Self, AttributeError> {
        self.attribute_flags.insert(GeometryFlags::JOINTS_0);
        self.update_attributes(|attr, joints| attr.joints_0 = joints, joints_0, 0)
    }

    /// Set/update the vertices joint weights. Used for skinning.
    pub fn weights_0(
        mut self,
        weights_0: impl IntoIterator<Item = Vec4>,
    ) -> Result<Self, AttributeError> {
        self.attribute_flags.insert(GeometryFlags::WEIGHTS_0);
        self.update_attributes(|attr, weights| attr.weights_0 = weights, weights_0, 0)
    }

    fn update_morph_target_attributes<Values: IntoIterator>(
        self,
        target: usize,
        update: impl FnMut(&mut Attribute, Values::Item),
        values: Values,
    ) -> Result<Self, MorphAttributeError> {
        if target < self.morph_target_count {
            self.update_attributes(update, values, 1 + target).map_err(
                |AttributeError::InvalidIterator { expected_length }| {
                    MorphAttributeError::InvalidIterator { expected_length }
                },
            )
        } else {
            Err(MorphAttributeError::InvalidMorphTarget {
                morph_target_count: self.morph_target_count,
            })
        }
    }

    /// Set/update the morph target positions.
    pub fn morph_target_positions(
        self,
        target: usize,
        positions: impl IntoIterator<Item = Vec3>,
    ) -> Result<Self, MorphAttributeError> {
        self.update_morph_target_attributes(target, |attr, pos| attr.position = pos, positions)
    }

    /// Set/update the morph target normals.
    pub fn morph_target_normals(
        self,
        target: usize,
        normals: impl IntoIterator<Item = Vec3>,
    ) -> Result<Self, MorphAttributeError> {
        self.update_morph_target_attributes(target, |attr, normal| attr.normal = normal, normals)
    }

    /// Set/update the morph target tangets. Used for normal mapping.
    pub fn morph_target_tangents(
        self,
        target: usize,
        tangents: impl IntoIterator<Item = Vec3>,
    ) -> Result<Self, MorphAttributeError> {
        self.update_morph_target_attributes(
            target,
            |attr, tangent| attr.tangent = tangent.extend(0.0),
            tangents,
        )
    }

    /// Set/update the morph target first texture coordinates.
    pub fn morph_target_tex_coords_0(
        self,
        target: usize,
        tex_coords_0: impl IntoIterator<Item = Vec2>,
    ) -> Result<Self, MorphAttributeError> {
        self.update_morph_target_attributes(target, |attr, tc| attr.tex_coord_0 = tc, tex_coords_0)
    }

    /// Set/update the morph target second texture coordinates.
    pub fn morph_target_tex_coords_1(
        self,
        target: usize,
        tex_coords_1: impl IntoIterator<Item = Vec2>,
    ) -> Result<Self, MorphAttributeError> {
        self.update_morph_target_attributes(target, |attr, tc| attr.tex_coord_1 = tc, tex_coords_1)
    }

    /// Set/update the morph target colors.
    pub fn morph_target_colors_0(
        self,
        target: usize,
        colors_0: impl IntoIterator<Item = Vec4>,
    ) -> Result<Self, MorphAttributeError> {
        self.update_morph_target_attributes(target, |attr, color| attr.color_0 = color, colors_0)
    }

    /// Set/update the morph target joints. Used for skinning.
    pub fn morph_target_joints_0(
        self,
        target: usize,
        joints_0: impl IntoIterator<Item = UVec4>,
    ) -> Result<Self, MorphAttributeError> {
        self.update_morph_target_attributes(target, |attr, joints| attr.joints_0 = joints, joints_0)
    }

    /// Set/update the morph target joint weights. Used for skinning.
    pub fn morph_target_weights_0(
        self,
        target: usize,
        weights_0: impl IntoIterator<Item = Vec4>,
    ) -> Result<Self, MorphAttributeError> {
        self.update_morph_target_attributes(
            target,
            |attr, weights| attr.weights_0 = weights,
            weights_0,
        )
    }

    /// Set/update the texture coordinates set that should be used for tangent generation.
    /// This must be set when using texture mapping with a geometry that doesn't have tangents.
    pub fn normal_tex_coord(mut self, normal_tex_coord: impl Into<u32>) -> Self {
        self.normal_tex_coord = Some(normal_tex_coord.into());
        self
    }

    /// Translates the geometry in its local space. In other words, `translation` is added the position
    /// of all the geometry vertices.
    ///
    /// This functions only modifies the positions attribute ([`GeometryBuilder::positions()`]).
    /// Only the base geometry is translated, morph target positions are not affected.
    /// Future call to [`GeometryBuilder::positions()`] will override the translation.
    pub fn translate(mut self, translation: impl Into<Vec3>) -> Self {
        let delta = translation.into();
        self.attributes
            .iter_mut()
            .take(self.vertex_count)
            .for_each(|a| a.position += delta);
        self
    }

    /// Scales the geometry in its local space. In other words, the position of all the geometry vertices is multiplied by `scale`.
    ///
    /// This functions only modifies the positions attribute ([`GeometryBuilder::positions()`]). Only the base geometry is scaled, morph target positions are not affected.
    /// Future call to [`GeometryBuilder::positions()`] will override the scaling.
    pub fn scale(mut self, scale: impl Into<Vec3>) -> Self {
        let delta = scale.into();
        self.attributes
            .iter_mut()
            .take(self.vertex_count)
            .for_each(|a| a.position *= delta);
        self
    }

    /// Rotates the geometry in its local space around the origin. In other words, `rotation` is applied to the position, normal and tangent of all the geometry vertices.
    ///
    /// This functions only modifies the positions, normals and tangents attributes ([`GeometryBuilder::positions()`], [`GeometryBuilder::normals()`],
    /// [`GeometryBuilder::tangents()`]). Only the base geometry is rotated, morph target positions, normals and tangents are not affected.
    /// Future call to [`GeometryBuilder::positions()`], [`GeometryBuilder::normals()`] or [`GeometryBuilder::tangents()`] will override the rotation.
    pub fn rotate(mut self, rotation: impl Into<Quat>) -> Self {
        let rotation = rotation.into();
        self.attributes
            .iter_mut()
            .take(self.vertex_count)
            .for_each(|a| {
                a.position = rotation * a.position;
                a.normal = rotation * a.normal;
                a.tangent = a.tangent.with_xyz(rotation * a.tangent.xyz());
            });
        self
    }

    pub fn build(mut self, ctx: &Context) -> Result<Geometry, GeometryBuilderError> {
        if self.morph_target_count > MAX_MORPH_TARGET_COUNT {
            return Err(GeometryBuilderError::TooManyMorphTarget);
        }
        if !self.attribute_flags.contains(GeometryFlags::POSITION) {
            return Err(GeometryBuilderError::PositionsNotSet);
        }

        let generate_normals = match self.topology {
            wgpu::PrimitiveTopology::PointList
            | wgpu::PrimitiveTopology::LineList
            | wgpu::PrimitiveTopology::LineStrip => {
                self.normal_tex_coord = None;
                false
            }
            _ => {
                let generate = !self.attribute_flags.contains(GeometryFlags::NORMAL);
                self.attribute_flags.insert(GeometryFlags::NORMAL);
                generate
            }
        };
        if generate_normals {
            // ignore provided tangents
            self.attribute_flags.remove(GeometryFlags::TANGENT);
        } else if self.attribute_flags.contains(GeometryFlags::TANGENT) {
            // use provided tangents.
            self.normal_tex_coord = None;
        }

        if generate_normals || self.normal_tex_coord.is_some() {
            // we cannot generate normals and tangents with indexed geometries
            match self.indices.take() {
                None => (),
                Some(indices) => {
                    (self.vertex_count, self.attributes) = match indices {
                        Indices::U16(indices) => {
                            let mut new_attributes =
                                Vec::with_capacity((1 + self.morph_target_count) * indices.len());

                            for target in 0..=self.morph_target_count {
                                let start = target * self.vertex_count;
                                let end = start + self.vertex_count;
                                let old_attr = &self.attributes[start..end];

                                new_attributes
                                    .extend(indices.iter().map(|idx| old_attr[*idx as usize]));
                            }

                            (indices.len(), new_attributes)
                        }
                        Indices::U32(indices) => {
                            let mut new_attributes =
                                Vec::with_capacity((1 + self.morph_target_count) * indices.len());

                            for target in 0..=self.morph_target_count {
                                let start = target * self.vertex_count;
                                let end = start + self.vertex_count;
                                let old_attr = &self.attributes[start..end];

                                new_attributes
                                    .extend(indices.iter().map(|idx| old_attr[*idx as usize]));
                            }

                            (indices.len(), new_attributes)
                        }
                    };
                }
            };

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

                    let mut mikk_tspace = MikkTSpace {
                        attributes,
                        normal_tex_coord,
                    };
                    mikktspace::generate_tangents(&mut mikk_tspace);
                }
                self.attribute_flags.insert(GeometryFlags::TANGENT);
            }
        }

        let header = GeometryStorageHeader {
            vertex_count: self.vertex_count as u32,
            target_count: self.morph_target_count as u32,
            _pad: [0; 2],
        };

        let header_size = size_of::<GeometryStorageHeader>();
        let size = header_size
            + (1 + self.morph_target_count) * self.vertex_count * size_of::<Attribute>();

        let vertex_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Geometry vertex buffer"),
            size: wgpu::util::align_to(size as u64, wgpu::COPY_BUFFER_ALIGNMENT),
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: true,
        });

        let header_size = header_size as wgpu::BufferAddress;
        let size = size as wgpu::BufferAddress;
        vertex_buffer
            .slice(0..header_size)
            .get_mapped_range_mut()
            .copy_from_slice(bytes_of(&header));
        if header_size < size {
            vertex_buffer
                .slice(header_size..size)
                .get_mapped_range_mut()
                .copy_from_slice(cast_slice(&self.attributes));
        }
        vertex_buffer.unmap();

        let indices = self.indices.map(|indices| {
            let (contents, format, count) = match &indices {
                Indices::U16(indices) => (
                    cast_slice(indices),
                    wgpu::IndexFormat::Uint16,
                    indices.len(),
                ),
                Indices::U32(indices) => (
                    cast_slice(indices),
                    wgpu::IndexFormat::Uint32,
                    indices.len(),
                ),
            };

            let buffer = ctx
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Geometry index buffer"),
                    contents,
                    usage: wgpu::BufferUsages::INDEX,
                });

            GeometryIndices {
                vector: indices,
                buffer,
                format,
                count,
            }
        });

        let attributes = GeometryAttributes {
            vector: self.attributes,
            buffer: vertex_buffer,
            flags: self.attribute_flags,
            vertex_count: self.vertex_count,
            morph_target_count: self.morph_target_count,
        };

        Ok(Geometry {
            attributes,
            indices,
            topology: self.topology,
        })
    }
}

#[derive(Debug, Clone, Error)]
pub enum AttributeError {
    #[error("attribute iterator must yield at least {expected_length} elements")]
    InvalidIterator { expected_length: usize },
}

#[derive(Debug, Clone, Error)]
pub enum MorphAttributeError {
    #[error("morph attribute iterator must yield at least {expected_length} elements")]
    InvalidIterator { expected_length: usize },
    #[error("geometry only has {morph_target_count} morph targets")]
    InvalidMorphTarget { morph_target_count: usize },
}

#[derive(Debug, Clone, Error)]
pub enum GeometryBuilderError {
    #[error("cannot have more than {MAX_MORPH_TARGET_COUNT} morph targets")]
    TooManyMorphTarget,
    #[error("position attribute is not set")]
    PositionsNotSet,
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

#[derive(Debug, Clone)]
enum Indices {
    U16(Vec<u16>),
    U32(Vec<u32>),
}

#[derive(Debug, Clone)]
struct GeometryAttributes {
    vector: Vec<Attribute>,
    buffer: wgpu::Buffer,
    flags: GeometryFlags,
    vertex_count: usize,
    morph_target_count: usize,
}

/// Holds geometry indices data.
#[derive(Debug, Clone)]
pub(super) struct GeometryIndices {
    /// `Vec` containing the indices.
    vector: Indices,

    /// GPU buffer containing the indices data.
    buffer: wgpu::Buffer,

    /// The indices format used by [`Self::buffer`].
    format: wgpu::IndexFormat,

    /// The number of vertices. For triangle-based geometry, this will be a multiple of `3`.
    count: usize,
}

impl GeometryIndices {
    /// GPU buffer containing the indices data.
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    /// The indices format used by [`Self::buffer`].
    pub fn format(&self) -> wgpu::IndexFormat {
        self.format
    }

    /// The number of vertices. For triangle-based geometry, this will be a multiple of `3`.
    pub fn count(&self) -> usize {
        self.count
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct GeometryFlags: u8 {
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

impl<'a> mikktspace::Geometry for MikkTSpace<'a> {
    fn num_faces(&self) -> usize {
        self.attributes.len() / 3
    }

    fn num_vertices_of_face(&self, _face: usize) -> usize {
        3
    }

    fn position(&self, face: usize, vert: usize) -> [f32; 3] {
        self.attribute(face, vert).position.to_array()
    }

    fn normal(&self, face: usize, vert: usize) -> [f32; 3] {
        self.attribute(face, vert).normal.to_array()
    }

    fn tex_coord(&self, face: usize, vert: usize) -> [f32; 2] {
        match self.normal_tex_coord {
            0 => self.attribute(face, vert).tex_coord_0.to_array(),
            1 => self.attribute(face, vert).tex_coord_1.to_array(),
            _ => unreachable!(),
        }
    }

    fn set_tangent_encoded(&mut self, tangent: [f32; 4], face: usize, vert: usize) {
        self.attribute_mut(face, vert).tangent =
            vec4(tangent[0], tangent[1], tangent[2], -tangent[3]);
    }
}
