use std::{collections::HashMap, time::Duration, u32};

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use thiserror::Error;

use crate::{
    Context, GpuCommandQueue,
    environment::{Environment, EnvironmentBuilder},
    mesh::{MeshInstance, MeshInstanceId},
    scene::{light::LightManager, skin::SkinManager},
    scene_graph::SceneGraph,
};

pub mod camera;
pub mod light;
pub mod renderer;
pub mod scene_graph;
pub mod skin;

/// A scene describes a world. A scene can be evolve over time and can be rendered
/// to a screen or a texture.
///
/// A scene is made up of nodes. Nodes are organized in a parent-child hierachy, known as the
/// node-hierarchy or the scene graph. A node is called a root node when it doesn't have a parent.
/// Each node defines a local space. The local transform is used to get from the parent node local
/// space (parent space for short) to the local space. The global transform is used to get from the
/// scene space (or global space) to the local space. Both transforms are equal for root nodes.
///
/// To add an object to the scene, attach it to a node. For example, each node can have a mesh. During
/// rendering, the attached mesh will be rendered at the local space origin.
#[derive(Debug)]
pub struct Scene {
    pub name: String,
    pub scene_graph: SceneGraph,
    ctx: Context,
    skin_manager: SkinManager,
    pub mesh_instances: HashMap<MeshInstanceId, MeshInstance>,
    light_manager: LightManager,
    environment: Environment,
}

impl Scene {
    pub fn context(&self) -> &Context {
        &self.ctx
    }

    pub fn skin_manager(&self) -> &SkinManager {
        &self.skin_manager
    }

    pub fn light_manager(&self) -> &LightManager {
        &self.light_manager
    }

    pub fn environment(&self) -> &Environment {
        &self.environment
    }

    pub fn simulate(
        &mut self,
        _duration: Duration,
        _encoder: &mut wgpu::CommandEncoder,
    ) -> Result<(), SimulateError> {
        self.light_manager
            .update_point_light_buffer(&self.scene_graph, &self.ctx.device, &self.ctx.queue)
            .unwrap();

        self.skin_manager
            .update_buffer(&self.scene_graph, &self.ctx.device, &self.ctx.queue)
            .unwrap();

        Ok(())
    }
}

/// A builder for [Scene].
#[must_use]
#[derive(Default)]
pub struct SceneBuilder {
    name: String,
    environment: Option<Environment>,
}

impl SceneBuilder {
    pub fn name(self, name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..self
        }
    }

    pub fn environment(mut self, environment: impl Into<Environment>) -> Self {
        self.environment = Some(environment.into());
        self
    }

    pub fn build(self, gpu_command_queue: &mut GpuCommandQueue) -> Scene {
        let ctx = gpu_command_queue.context().clone();
        let scene_graph = SceneGraph::new(&ctx);
        let skin_manager = SkinManager::new(&ctx);
        let light_manager = LightManager::new(&ctx);

        let environment = self
            .environment
            .unwrap_or_else(|| EnvironmentBuilder::default().build(gpu_command_queue));

        Scene {
            name: self.name,
            ctx,
            scene_graph,
            skin_manager,
            mesh_instances: HashMap::new(),
            light_manager,
            environment,
        }
    }
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct CameraUniform {
    view_projection: Mat4,
    view: Mat4,
    projection_inverse: Mat4,
    position: Vec3,
    _pad: u32,
}

#[derive(Debug, Error)]
pub enum SimulateError {}
