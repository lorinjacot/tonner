use glam::{Quat, Vec3};

/// A transform is a combination of a translation and a rotation. The translation is always applied before the rotation.
#[derive(Debug, Clone)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
}

impl Transform {
    /// The identity transformation. Corresponds to no translation and no rotation.
    ///
    /// ## Example
    /// ```
    /// # use glam::{Vec3, Quat};
    /// # use tonner::Transform;
    /// assert_eq!(Transform::IDENTITY.translation, Vec3::ZERO);
    /// assert_eq!(Transform::IDENTITY.rotation, Quat::IDENTITY);
    /// ```
    pub const IDENTITY: Transform = Transform {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
    };

    /// Creates a `Transform` from the given `translation` and a unit rotation. This results in a pure translation.
    ///
    /// ## Example
    ///
    /// ```
    /// # use glam::{vec3, Quat};
    /// # use tonner::Transform;
    /// let translation = vec3(1.0, 2.0, 3.0);
    /// let transform = Transform::from_translation(translation);
    /// assert_eq!(transform.translation, translation);
    /// assert_eq!(transform.rotation, Quat::IDENTITY);
    /// ```
    pub fn from_translation(translation: Vec3) -> Transform {
        Transform {
            translation,
            rotation: Quat::IDENTITY,
        }
    }

    /// Creates a `Transform` from the given `rotation` and zero translation. This results in a pure rotation.
    ///
    /// ## Example
    /// ```
    /// # use glam::{Vec3, Quat};
    /// # use tonner::Transform;
    /// let rotation = Quat::from_rotation_y(1.0);
    /// let transform = Transform::from_rotation(rotation);
    /// assert_eq!(transform.translation, Vec3::ZERO);  
    /// assert_eq!(transform.rotation, rotation);
    /// ```
    pub fn from_rotation(rotation: Quat) -> Transform {
        Transform {
            translation: Vec3::ZERO,
            rotation,
        }
    }
}
