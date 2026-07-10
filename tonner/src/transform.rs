use glam::{DQuat, DVec3};

/// A transform is a combination of a translation and a rotation. The translation is always applied before the rotation.
#[derive(Debug, Clone)]
pub struct Transform {
    pub translation: DVec3,
    pub rotation: DQuat,
}

impl Transform {
    /// The identity transformation. Corresponds to no translation and no rotation.
    ///
    /// ## Example
    /// ```
    /// # use glam::{DVec3, DQuat};
    /// # use tonner::Transform;
    /// assert_eq!(Transform::IDENTITY.translation, DVec3::ZERO);
    /// assert_eq!(Transform::IDENTITY.rotation, DQuat::IDENTITY);
    /// ```
    pub const IDENTITY: Transform = Transform {
        translation: DVec3::ZERO,
        rotation: DQuat::IDENTITY,
    };

    /// Creates a `Transform` from the given `translation` and a unit rotation. This results in a pure translation.
    ///
    /// ## Example
    ///
    /// ```
    /// # use glam::{DVec3, DQuat};
    /// # use tonner::Transform;
    /// let translation = DVec3::new(1.0, 2.0, 3.0);
    /// let transform = Transform::from_translation(translation);
    /// assert_eq!(transform.translation, translation);
    /// assert_eq!(transform.rotation, DQuat::IDENTITY);
    /// ```
    pub fn from_translation(translation: DVec3) -> Transform {
        Transform {
            translation,
            rotation: DQuat::IDENTITY,
        }
    }

    /// Creates a `Transform` from the given `rotation` and zero translation. This results in a pure rotation.
    ///
    /// ## Example
    /// ```
    /// # use glam::{DVec3, DQuat};
    /// # use tonner::Transform;
    /// let rotation = DQuat::from_rotation_y(1.0);
    /// let transform = Transform::from_rotation(rotation);
    /// assert_eq!(transform.translation, DVec3::ZERO);  
    /// assert_eq!(transform.rotation, rotation);
    /// ```
    pub fn from_rotation(rotation: DQuat) -> Transform {
        Transform {
            translation: DVec3::ZERO,
            rotation,
        }
    }
}
