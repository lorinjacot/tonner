use crate::{gltf::GltfAsset, Resources, Scene};

pub fn open_gltf<'r>(
    path: impl AsRef<std::path::Path>,
    resources: &'r mut Resources,
    encoder: &mut wgpu::CommandEncoder,
    render_width: u32,
    render_height: u32,
) -> anyhow::Result<(Vec<Scene>, Option<usize>)> {
    let mut asset = GltfAsset::open(&path)?;
    let scene = asset.load_scene(0, resources, encoder, render_width, render_height)?;

    Ok((vec![scene], Some(0)))
}
