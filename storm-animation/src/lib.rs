use std::{
    collections::{HashMap, hash_map::Entry},
    fmt::Debug,
    time::Duration,
};

use tonner::{
    mesh::{MeshInstance, MeshInstanceId},
    scene_graph::SceneGraph,
};
use uuid::Uuid;

pub mod key_frame;

#[derive(Debug, Default)]
pub struct AnimationManager {
    running_animations: HashMap<AnimationId, Animation>,
    stopped_animations: HashMap<AnimationId, Animation>,
}

impl AnimationManager {
    /// Adds `animation` to the manager and returns its id.
    /// By default, the animation is stopped. To start it,
    /// use [`AnimationManager::start()`].
    pub fn insert(&mut self, animation: Animation) -> AnimationId {
        let id = AnimationId(Uuid::new_v4());
        self.stopped_animations.insert(id, animation);
        id
    }

    /// Remove and returns the animation from the manager. It does
    /// not if the animation is currently running or not.
    /// Returns `None` if the animation does not exist.
    pub fn remove(&mut self, id: AnimationId) -> Option<Animation> {
        self.stopped_animations
            .remove(&id)
            .or_else(|| self.running_animations.remove(&id))
    }

    /// This function advance all running animations by `delta_time`. An animation
    /// can modify any field of `animatable`. This function should be called once per
    /// frame.
    pub fn update(&mut self, delta_time: Duration, animatable: &mut Animatable) {
        let mut animations_to_stop = Vec::new();
        for (&id, animation) in &mut self.running_animations {
            animation.progress += delta_time;
            if animation.progress >= animation.duration {
                if animation.repeat {
                    animation.progress -= animation.duration;
                    while animation.progress >= animation.duration {
                        animation.progress -= animation.duration;
                    }
                } else {
                    animation.progress = Duration::ZERO;
                    animations_to_stop.push(id);
                    continue;
                }
            }
            for channel in &mut animation.channels {
                channel.update(animation.progress, animation.duration, animatable);
            }
        }
        animations_to_stop.into_iter().for_each(|id| {
            self.stopped_animations
                .insert(id, self.running_animations.remove(&id).unwrap());
        });
    }

    /// Starts the animation if not currently running.
    /// Does nothing and returns `Ok` if the animation is already running.
    /// Returns `Err` if the animation does not exist.
    pub fn start(&mut self, id: AnimationId) -> Result<(), ()> {
        match self.stopped_animations.remove(&id) {
            Some(animation) => {
                self.running_animations.insert(id, animation);
                Ok(())
            }
            None if self.running_animations.contains_key(&id) => Ok(()),
            _ => Err(()),
        }
    }

    /// Start the animation from the beginning and returns `Ok`. It does not
    /// matter if the animation is already running or not.
    /// Returns `Err` if the animation does not exist.
    pub fn restart(&mut self, id: AnimationId) -> Result<(), ()> {
        match self.running_animations.entry(id) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().progress = Duration::ZERO;
                Ok(())
            }
            Entry::Vacant(entry) => match self.stopped_animations.remove(&id) {
                Some(mut animation) => {
                    animation.progress = Duration::ZERO;
                    entry.insert(animation);
                    Ok(())
                }
                None => Err(()),
            },
        }
    }

    /// Stops the animation if currently running while leaving [`Animation::progress`] unchanged
    /// if it is running.
    /// Does nothing and returns `Ok` if the animation is already stopped.
    /// Returns `Err` if the animation does not exist.
    pub fn pause(&mut self, id: AnimationId) -> Result<(), ()> {
        match self.running_animations.remove(&id) {
            Some(animation) => {
                self.stopped_animations.insert(id, animation);
                Ok(())
            }
            None if self.stopped_animations.contains_key(&id) => Ok(()),
            _ => Err(()),
        }
    }

    /// Stops the animation if currently running and sets [`Animation::progress`] to `Duration::ZERO`.
    /// Does nothing and returns `Ok` if the animation exists but is not running.
    /// Returns `Err` if the animation does not exist.
    pub fn stop(&mut self, id: AnimationId) -> Result<(), ()> {
        match self.running_animations.remove(&id) {
            Some(mut animation) => {
                animation.progress = Duration::ZERO;
                self.stopped_animations.insert(id, animation);
                Ok(())
            }
            None if self.stopped_animations.contains_key(&id) => Ok(()),
            _ => Err(()),
        }
    }

    /// Returns `true` if the animation exists, `false` otherwise.
    pub fn contains(&self, id: AnimationId) -> bool {
        self.running_animations.contains_key(&id) || self.stopped_animations.contains_key(&id)
    }

    /// Returns a reference to the animation or `None` if it does not exist.
    pub fn get(&self, id: AnimationId) -> Option<&Animation> {
        self.running_animations
            .get(&id)
            .or_else(|| self.stopped_animations.get(&id))
    }

    /// Returns a mutable reference to the animation or `None` if it does not exist.
    pub fn get_mut(&mut self, id: AnimationId) -> Option<&mut Animation> {
        self.running_animations
            .get_mut(&id)
            .or_else(|| self.stopped_animations.get_mut(&id))
    }

    /// Returns an iterator over both running and stopped animations.
    pub fn iter(&self) -> impl Iterator<Item = (&AnimationId, &Animation)> {
        self.running_animations
            .iter()
            .chain(self.stopped_animations.iter())
    }

    /// Returns an iterator over mutable references of both running and stopped animations.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&AnimationId, &mut Animation)> {
        self.running_animations
            .iter_mut()
            .chain(self.stopped_animations.iter_mut())
    }

    /// All currently running animations.
    pub fn running_animations(&self) -> impl Iterator<Item = (&AnimationId, &Animation)> {
        self.running_animations.iter()
    }

    /// All currently running animations as mutable references.
    pub fn running_animations_mut(
        &mut self,
    ) -> impl Iterator<Item = (&AnimationId, &mut Animation)> {
        self.running_animations.iter_mut()
    }

    /// All currently stopped animations.
    pub fn stopped_animations(&self) -> impl Iterator<Item = (&AnimationId, &Animation)> {
        self.stopped_animations.iter()
    }

    /// All currently stopped running animations as mutable references.
    pub fn stopped_animations_mut(
        &mut self,
    ) -> impl Iterator<Item = (&AnimationId, &mut Animation)> {
        self.stopped_animations.iter_mut()
    }
}

/// A unique id for [`Animation`]. An `animation` has one and only one id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnimationId(Uuid);

#[derive(Debug)]
pub struct Animation {
    /// Name of the animation. Does not need to be unique. Can be used for debugging and displaying.
    pub name: String,
    /// The channels contains the actual animation data.
    pub channels: Vec<Box<dyn AnimationChannel>>,
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
    pub mesh_instance: &'a mut HashMap<MeshInstanceId, MeshInstance>,
}

pub trait AnimationChannel: Debug + Send + Sync {
    fn update(&mut self, progress: Duration, duration: Duration, animatable: &mut Animatable);
}
