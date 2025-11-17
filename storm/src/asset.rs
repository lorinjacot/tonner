use crate::{Resources, Scene, gltf::GltfAsset};

pub mod mesh;
pub mod geometry;

pub fn open_gltf<'r>(
    path: impl AsRef<std::path::Path>,
    resources: &'r mut Resources,
    encoder: &mut wgpu::CommandEncoder,
    render_width: u32,
    render_height: u32,
) -> anyhow::Result<(Vec<Scene>, Option<usize>)> {
    let mut asset = GltfAsset::open(&path)?;

    let mut scene = Scene::new(
        path.as_ref().to_str().unwrap_or_default().to_string(),
        resources,
        encoder,
        render_width,
        render_height,
    );
    asset.load_scene_into(0, resources, encoder, &mut scene, None)?;

    Ok((vec![scene], Some(0)))
}
