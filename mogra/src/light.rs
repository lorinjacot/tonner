use std::collections::HashMap;

use glam::Vec3;

use crate::{Context, NodeId};

#[derive(Debug, Clone)]
pub struct PointLight {
    color: Vec3,
}

#[derive(Debug, Clone)]
pub(super) struct LightManager {
    point_lights: HashMap<NodeId, PointLight>,
}

impl LightManager {
    pub fn new(ctx: &Context) -> Self {
        let point_lights = HashMap::new();

        LightManager { point_lights }
    }
}
