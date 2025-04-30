use glam::{Mat4, Quat};

use crate::{
    Id, MaterialBuilder, Resources,
    mesh::{Indices, Mesh},
    scene::{Node, Scene},
    storage::{DenseEntry, SparseSet},
};

pub fn open_gltf<'r>(
    path: impl AsRef<std::path::Path>,
    resources: &'r mut Resources,
    encoder: &mut wgpu::CommandEncoder,
) -> Result<(Vec<Scene>, Option<usize>), gltf::Error> {
    let (document, buffers, _images) = gltf::import(path)?;

    let materials: Vec<_> = document
        .materials()
        .map(|material| resources.material_builder().from_gltf(material).build())
        .collect();

    let mesh_mapping: Vec<_> = document
        .meshes()
        .map(|mesh| {
            let mut primitives = Vec::with_capacity(mesh.primitives().len());
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
                if let Some(position) = reader.read_positions() {
                    if reader.read_normals().is_none() {
                        todo!("generate normals")
                    }
                    let material = primitive.material();
                    let material = match material.index() {
                        Some(index) => &materials[index],
                        None => &resources.material_builder().from_gltf(material).build(),
                    };
                    let mut primitive_builder = resources.primitive_builder();
                    let indices;
                    primitive_builder = match reader.read_indices() {
                        Some(indices_reader) => {
                            indices = indices_reader.into_u32().collect::<Vec<_>>();
                            primitive_builder
                                .vertex_count(indices.len() as u32)
                                .indices(Indices::Slice(&indices))
                        }
                        None => primitive_builder.vertex_count(position.len() as u32),
                    };
                    let primitive = primitive_builder
                        .positions(Some(&position.collect::<Vec<_>>()))
                        .normals(
                            reader
                                .read_normals()
                                .map(|normals| normals.collect::<Vec<_>>())
                                .as_deref(),
                        )
                        .material(material)
                        .build();
                    primitives.push(primitive);
                }
            }
            resources
                .mesh_builder()
                .name(mesh.name().map(|name| name.to_string()))
                .primitives(primitives)
                .build()
                .id()
        })
        .collect();

    let scenes = document
        .scenes()
        .map(|gltf_scene| {
            let mut scene = Scene::new(
                gltf_scene
                    .name()
                    .map_or_else(|| gltf_scene.index().to_string(), |name| name.to_string()),
                resources,
                encoder,
            );
            for node in gltf_scene.nodes() {
                scene.build_gltf_node(node, None, &mut resources.meshes, &mesh_mapping);
            }
            scene
        })
        .collect();

    let default_scene = document.default_scene().map(|scene| scene.index());
    Ok((scenes, default_scene))
}

impl<'r> MaterialBuilder<'r> {
    fn from_gltf(self, material: gltf::Material) -> Self {
        let pbr_metallic_roughness = material.pbr_metallic_roughness();
        self.base_color_factor(pbr_metallic_roughness.base_color_factor())
            .metallic_factor(pbr_metallic_roughness.metallic_factor())
            .roughness_factor(pbr_metallic_roughness.roughness_factor())
    }
}

impl Scene {
    fn build_gltf_node(
        &mut self,
        node: gltf::Node,
        parent: Option<Id<Node>>,
        meshes: &mut SparseSet<Mesh>,
        mesh_mapping: &[Id<Mesh>],
    ) -> Id<Node> {
        let mesh = node
            .mesh()
            .map(|gltf_mesh| &meshes[mesh_mapping[gltf_mesh.index()]]);
        let mut builder = self
            .node_builder()
            .name(node.name().map(|name| name.to_string()))
            .parent(parent);
        builder = match node.transform() {
            gltf::scene::Transform::Decomposed {
                translation,
                rotation,
                scale,
            } => builder.translation_rotation_scale(
                translation.into(),
                Quat::from_array(rotation),
                scale.into(),
            ),
            gltf::scene::Transform::Matrix { matrix } => {
                builder.local_matrix(Mat4::from_cols_array_2d(&matrix))
            }
        };
        let id = builder.mesh(mesh).build().id();
        for child in node.children() {
            self.build_gltf_node(child, Some(id), meshes, mesh_mapping);
        }
        id
    }
}
