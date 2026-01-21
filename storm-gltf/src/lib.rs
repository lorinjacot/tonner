use anyhow::{Context, Result, anyhow};
use bytemuck::bytes_of_mut;
use data_url::forgiving_base64::InvalidBase64;
use serde::{Deserialize, Serialize};
use std::{
    fmt::Display,
    fs::File,
    io::{BufReader, Read, Seek},
    path::{Path, PathBuf},
};
use storm::{SceneBuilder, environment::Environment};
use thiserror::Error;

use accessor::{Accessor, AccessorComponentType, AccessorType};
use animation::Animation;
use buffer::{Buffer, BufferView};
use material::Material;
use mesh::Mesh;
use scene::{Node, Scene, Skin};
use texture::{Image, Sampler, Texture};

mod accessor;
mod animation;
mod buffer;
mod material;
mod mesh;
mod scene;
mod texture;
mod transforms;

#[derive(Error, Debug)]
pub enum GltfError {
    #[error("Invalid binary gltf container: {0}")]
    Glb(#[from] GlbError),
    #[error("Failed to parse json: {0}")]
    Json(#[from] serde_json::Error),
    #[error(
        "{usage} cannot have accessor with {accessor_type} of {component_type} (normalized: {normalized})"
    )]
    InvalidAccessorDataType {
        accessor_type: AccessorType,
        component_type: AccessorComponentType,
        normalized: bool,
        usage: AccessorUsage,
    },
    #[error("Failed to read external image: {0}")]
    Image(#[from] image::ImageError),
    #[error("Invalid base64 encoding")]
    InvalidBase64(#[from] InvalidBase64),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug)]
pub enum AccessorUsage {
    Indices,
}

impl Display for AccessorUsage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Indices => write!(f, "Primitive indices"),
        }
    }
}

#[derive(Error, Debug)]
pub enum GlbError {
    #[error("Binary glTF container version should be 2")]
    UnknownVersion,
    #[error("A glb asset must have a JSON chunk as first chunk")]
    JsonChunkMissing,
    #[error("This glb asset must have a BIN chunk as second chunk")]
    BinChunkMissing,
    #[error("Invalid chunk length")]
    InvalidChunkLength,
}

#[derive(Debug)]
pub struct GltfAsset {
    base_path: PathBuf,
    json: Gltf,
    default_material: Option<storm::material::Material>,
}

impl GltfAsset {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let read_failed_ctx = || format!("Failed to read asset from {path:?}");

        let mut file = File::open(path).with_context(read_failed_ctx)?;
        let base_path = path
            .parent()
            .expect("a file should be located in a folder")
            .to_owned();

        let mut magic: u32 = 0;
        file.read_exact(bytes_of_mut(&mut magic))
            .with_context(read_failed_ctx)?;
        let mut json = if magic == GLTF {
            let mut reader = BufReader::new(file);

            let mut version: u32 = 0;
            reader
                .read_exact(bytes_of_mut(&mut version))
                .with_context(read_failed_ctx)?;
            if version != 2 {
                return Err(GlbError::UnknownVersion.into());
            }

            let mut length: u32 = 0;
            reader
                .read_exact(bytes_of_mut(&mut length))
                .with_context(read_failed_ctx)?;
            length -= GLB_HEADER_SIZE;
            let mut reader = reader.take(length as u64);

            let json = GlbChunk::from_reader(&mut reader, &read_failed_ctx)?;
            if json.chunk_type != JSON {
                return Err(GlbError::JsonChunkMissing.into());
            }
            let mut json: Gltf = serde_json::from_slice(&json.chunk_data)?;

            if let Some(buffer) = json.buffers.first_mut() {
                if buffer.uri().is_none() {
                    let bin = GlbChunk::from_reader(&mut reader, &read_failed_ctx)?;
                    if bin.chunk_type != BIN {
                        return Err(GlbError::BinChunkMissing.into());
                    }
                    *buffer.bytes_mut() = bin.chunk_data;
                }
            }
            json
        } else {
            file.rewind().with_context(read_failed_ctx)?;

            let mut json = String::new();
            file.read_to_string(&mut json)
                .with_context(read_failed_ctx)?;

            serde_json::from_str(&json)?
        };

        for (idx, buffer) in json.buffers.iter_mut().enumerate() {
            if let Some(uri) = &buffer.uri() {
                let path = base_path.join(uri);

                let read_failed_ctx = || format!("Failed to read binary buffer from {path:?}");
                let mut file = File::open(&path).with_context(read_failed_ctx)?;
                file.read_to_end(buffer.bytes_mut())
                    .with_context(read_failed_ctx)?;
                if buffer.bytes().len() < buffer.byte_length() {
                    return Err(
                        anyhow!("the byte lenght of the referenced resource must be greater than or equal to the buffer.byte_length property")
                    ).with_context(|| format!("Failed to load buffer {idx}"));
                }
            }
        }

        Ok(Self {
            base_path,
            json,
            default_material: None,
        })
    }

    pub fn load_meshes(
        &mut self,
        ctx: &storm::Context,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<Vec<storm::mesh::Mesh>> {
        let mut meshes = Vec::with_capacity(self.json.meshes.len());

        for mesh in self.json.meshes.iter_mut() {
            meshes.push(mesh.load(
                &self.base_path,
                &self.json.accessors,
                &mut self.json.materials,
                &mut self.default_material,
                &mut self.json.textures,
                &mut self.json.samplers,
                &mut self.json.images,
                &self.json.buffer_views,
                &self.json.buffers,
                ctx,
                encoder,
            )?);
        }

        Ok(meshes)
    }

    pub fn create_scenes(
        &mut self,
        default_environment: Option<&Environment>,
        ctx: &storm::Context,
    ) -> anyhow::Result<Vec<storm::Scene>> {
        let mut scenes: Vec<storm::Scene> = self
            .json
            .scenes
            .iter()
            .map(|scene| {
                let mut builder =
                    SceneBuilder::default().name(scene.name().clone().unwrap_or(String::new()));
                if let Some(environment) = default_environment {
                    builder = builder.environment(environment.clone())
                }
                builder.build(ctx)
            })
            .collect();

        let mut encoder = ctx
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("GltfAsset::create_scenes command encoder"),
            });
        for (scene_index, scene) in scenes.iter_mut().enumerate() {
            self.load_scene_into(scene_index, None, scene, ctx, &mut encoder)?;
        }
        ctx.queue().submit([encoder.finish()]);

        Ok(scenes)
    }

    pub fn default_scene(&self) -> Option<usize> {
        self.json.scene
    }
}

const GLB_HEADER_SIZE: u32 = 3 * size_of::<u32>() as u32;
const GLTF: u32 = 0x46546C67;
const JSON: u32 = 0x4E4F534A;
const BIN: u32 = 0x004E4942;

struct GlbChunk {
    chunk_type: u32,
    chunk_data: Vec<u8>,
}

impl GlbChunk {
    fn from_reader<R: Read>(reader: &mut R, read_failed_ctx: &impl Fn() -> String) -> Result<Self> {
        let mut chunk_length: u32 = 0;
        reader
            .read_exact(bytes_of_mut(&mut chunk_length))
            .with_context(read_failed_ctx)?;

        let mut chunk_type: u32 = 0;
        reader
            .read_exact(bytes_of_mut(&mut chunk_type))
            .with_context(read_failed_ctx)?;

        let mut chunk_data = Vec::with_capacity(chunk_length as usize);
        reader
            .take(chunk_length as u64)
            .read_to_end(&mut chunk_data)
            .with_context(read_failed_ctx)?;

        if (chunk_data.len() as u32) < chunk_length {
            return Err(GlbError::InvalidChunkLength.into());
        }

        Ok(Self {
            chunk_type,
            chunk_data,
        })
    }
}

/// The root object for a glTF asset.
#[derive(Debug, Serialize, Deserialize)]
struct Gltf {
    /// Names of glTF extensions used in this asset.
    #[serde(rename = "extensionsUsed")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    extensions_used: Vec<String>,

    /// Names of glTF extensions required to properly load this asset.
    #[serde(rename = "extensionsRequired")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    extensions_required: Vec<String>,

    /// An array of accessors. An accessor is a typed view into a bufferView.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    accessors: Vec<Accessor>,

    /// An array of keyframe animations.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    animations: Vec<Animation>,

    /// Metadata about the glTF asset.
    asset: Asset,

    /// An array of buffers. A buffer points to binary geometry, animation, or skins.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    buffers: Vec<Buffer>,

    /// An array of bufferViews. A bufferView is a view into a buffer generally representing a subset of the buffer.
    #[serde(rename = "bufferViews")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    buffer_views: Vec<BufferView>,

    /// An array of cameras. A camera defines a projection matrix.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    cameras: Vec<Camera>,

    /// An array of images. An image defines data used to create a texture.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    images: Vec<Image>,

    /// An array of materials. A material defines the appearance of a primitive.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    materials: Vec<Material>,

    /// An array of meshes. A mesh is a set of primitives to be rendered.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    meshes: Vec<Mesh>,

    /// An array of nodes.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    nodes: Vec<Node>,

    /// An array of samplers. A sampler contains properties for texture filtering and wrapping modes.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    samplers: Vec<Sampler>,

    /// The index of the default scene. This property **MUST NOT** be defined, when [scenes](Gltf::scenes) is undefined.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    scene: Option<usize>,

    /// An array of scenes.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    scenes: Vec<Scene>,

    /// An array of skins. A skin is defined by joints and matrices.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    skins: Vec<Skin>,

    /// An array of textures.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    textures: Vec<Texture>,
}

/// Metadata about the glTF asset.
#[derive(Debug, Serialize, Deserialize)]
struct Asset {
    /// A copyright message suitable for display to credit the content creator.
    copyright: Option<String>,

    /// Tool that generated this glTF model. Useful for debugging.
    generator: Option<String>,

    /// The glTF version in the form of `<major>.<minor>` that this asset targets.
    version: String,

    /// The minimum glTF version in the form of `<major>.<minor>` that this asset targets.
    /// This property **MUST NOT** be greater than the asset version.
    #[serde(rename = "minVersion")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    min_version: Option<String>,
}

///A camera’s projection. A node **MAY** reference a camera to apply a transform to place the camera in the scene.
#[derive(Debug, Serialize, Deserialize)]
struct Camera {
    /// An orthographic camera containing properties to create an orthographic projection matrix.
    /// This property **MUST NOT** be defined when [perspective](Camera::perspective) is defined.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    orthographic: Option<OrthographicCamera>,

    /// A perspective camera containing properties to create a perspective projection matrix.
    /// This property **MUST NOT** be defined when [orthographic](Camera::orthographic) is defined.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    perspective: Option<PerspectiveCamera>,

    /// Specifies if the camera uses a perspective or orthographic projection.
    /// Based on this, either the camera’s [perspective](Camera::perspective)
    /// or [orthographic](Camera::orthographic) property **MUST** be defined.
    #[serde(rename = "type")]
    type_: CameraType,

    /// The user-defined name of this object. This is not necessarily unique,
    /// e.g., an accessor and a buffer could have the same name, or two accessors
    /// could even have the same name.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

/// An orthographic camera containing properties to create an orthographic projection matrix.
#[derive(Debug, Serialize, Deserialize)]
struct OrthographicCamera {
    /// The floating-point horizontal magnification of the view. This value **MUST NOT**
    /// be equal to zero. This value **SHOULD NOT** be negative.
    xmag: f32,

    /// The floating-point vertical magnification of the view. This value **MUST NOT**
    /// be equal to zero. This value **SHOULD NOT** be negative.
    ymag: f32,

    /// The floating-point distance to the far clipping plane. This value **MUST NOT**
    /// be equal to zero. zfar **MUST** be greater than znear.
    zfar: f32,

    /// The floating-point distance to the near clipping plane.
    znear: f32,
}

/// A perspective camera containing properties to create a perspective projection matrix.
#[derive(Debug, Serialize, Deserialize)]
struct PerspectiveCamera {
    /// The floating-point aspect ratio of the field of view. When undefined, the aspect
    /// ratio of the rendering viewport **MUST** be used.
    #[serde(rename = "aspectRatio")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    aspect_ratio: Option<f32>,

    /// The floating-point vertical field of view in radians. This value **SHOULD** be less than π.
    yfov: f32,

    /// The floating-point distance to the far clipping plane. When defined, `zfar` **MUST** be greater
    /// than [znear](PerspectiveCamera::znear). If `zfar` is undefined, client implementations **SHOULD**
    /// use infinite projection matrix.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    zfar: Option<f32>,

    /// The floating-point distance to the near clipping plane.
    znear: f32,
}

/// Specifies if the camera uses a perspective or orthographic projection.
/// Based on this, either the camera’s [perspective](Camera::perspective)
/// or [orthographic](Camera::orthographic) property **MUST** be defined.
#[derive(Debug, Serialize, Deserialize)]
enum CameraType {
    #[serde(rename = "perspective")]
    Perspective,

    #[serde(rename = "orthographic")]
    Orthographic,
}
