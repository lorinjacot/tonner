use glam::{Mat4, Quat, Vec3};

#[derive(Debug, Clone)]
pub struct Transform {
    translation: Vec3,
    rotation: Quat,
    scale: Vec3,
    matrix: Mat4,
}

impl Transform {
    /// The identity transformation.
    pub const IDENTITY: Self = Self {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
        matrix: Mat4::IDENTITY,
    };

    /// The transform's scale.
    pub fn scale(&self) -> Vec3 {
        self.scale
    }

    /// The transform's scale.
    pub fn rotation(&self) -> Quat {
        self.rotation
    }

    /// The transform's translation
    pub fn translation(&self) -> Vec3 {
        self.translation
    }

    /// The transform's matrix.
    pub fn matrix(&self) -> Mat4 {
        self.matrix
    }

    /// Set the transfom's matrix
    pub fn set_matrix(&mut self, matrix: impl Into<Mat4>) {
        self.matrix = matrix.into();
        (self.scale, self.rotation, self.translation) = self.matrix.to_scale_rotation_translation();
    }

    /// A [Vec3] representing the transform's position
    pub fn position(&self) -> Vec3 {
        self.translation
    }

    /// Set the transform's position
    pub fn set_position(&mut self, position: impl Into<Vec3>) {
        self.translation = position.into();
        self.matrix =
            Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation);
    }

    pub fn set_scale(&mut self, scale: impl Into<Vec3>) {
        self.scale = scale.into();
        self.matrix =
            Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation);
    }

    pub fn set_rotation(&mut self, rotation: impl Into<Quat>) {
        self.rotation = rotation.into();
        self.matrix =
            Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation);
    }

    /// Rotates the transform such that it faces a point in world space.
    /// For a view coordinate system with `+X=right`, `+Y=up` and `+Z=back`
    pub fn look_at(&mut self, target: Vec3) {
        self.set_matrix(Mat4::look_at_rh(self.position(), target, Vec3::Y).inverse());
    }
}
