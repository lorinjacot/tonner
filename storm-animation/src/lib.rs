use std::{collections::HashMap, time::Duration};

use storm::scene_graph::{NodeNotFoundError, SceneGraph};
use thiserror::Error;
use uuid::Uuid;

#[derive(Default)]
pub struct AnimationManager {
    running_animations: HashMap<AnimationId, Animation>,
    stopped_animations: HashMap<AnimationId, Animation>,
}

impl AnimationManager {
    pub fn update(
        &mut self,
        delta_time: Duration,
        animatable: &mut Animatable,
    ) -> Result<(), (AnimationId, AnimationError)> {
        for (&id, animation) in &mut self.running_animations {
            animation.progress += delta_time;
            if animation.progress >= animation.duration {
                if animation.repeat {
                    animation.progress -= animation.duration;
                    while animation.progress >= animation.duration {
                        animation.progress -= animation.duration;
                    }
                } else {
                    todo!("stop animation");
                }
            }
            for channel in &mut animation.channel {
                channel
                    .update(animation.progress, animation.duration, animatable)
                    .map_err(|error| (id, error))?;
            }
        }
        Ok(())
    }

    pub fn running_animations(&self) -> impl Iterator<Item = (&AnimationId, &Animation)> {
        self.running_animations.iter()
    }

    pub fn running_animations_mut(
        &mut self,
    ) -> impl Iterator<Item = (&AnimationId, &mut Animation)> {
        self.running_animations.iter_mut()
    }
    pub fn stopped_animations(&self) -> impl Iterator<Item = (&AnimationId, &Animation)> {
        self.stopped_animations.iter()
    }

    pub fn stopped_animations_mut(
        &mut self,
    ) -> impl Iterator<Item = (&AnimationId, &mut Animation)> {
        self.stopped_animations.iter_mut()
    }
}

/// A unique id for [`Animation`]. An `animation` has one and only one id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnimationId(Uuid);

pub struct Animation {
    /// Name of the animation. Does not need to be unique. Can be used for debugging and displaying.
    pub name: String,
    pub channel: Vec<Box<dyn AnimationChannel>>,
    /// If `true`, the animation will restart once it has finished.
    pub repeat: bool,
    /// Current progress of the animation. This value can be set to `0` to restart the animation.
    pub progress: Duration,
    /// Total duration of the animation. If `duration <= progress`, the animataion will :
    /// - if `repeat == false`, the animation will be stopped on next [`AnimationManager::update()`] call.
    /// - if `repeat == true`, `duration` will be substracted from `progress` until `progress < duration`.
    pub duration: Duration,
}

pub struct Animatable<'a> {
    pub scene_graph: &'a mut SceneGraph,
}

pub trait AnimationChannel {
    fn update(
        &mut self,
        progress: Duration,
        duration: Duration,
        animatable: &mut Animatable,
    ) -> Result<(), AnimationError>;
}

/// Error when [`AnimationChannel::update()`] fails.
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum AnimationError {
    #[error(transparent)]
    NodeNotFound(#[from] NodeNotFoundError),
}
