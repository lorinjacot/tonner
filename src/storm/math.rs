use glam::{Mat4, Quat, Vec3};

pub const EPSILON: f32 = 1e-8;

#[derive(Clone, Copy)]
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

    /// The transform matrix.
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

/// A ray that emits from an origin in a certain direction.
pub struct Ray {
    /// The origin of the Ray
    pub origin: Vec3,

    /// The direction of the Ray. This must be normalized for the methods to operate properly
    pub direction: Vec3,
}

impl Ray {
    /// Returns a [Vec3] that is located at a given distance `t` along this ray.
    pub fn at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }

    /// Computes the distance from the ray's origin to the given plane. Returns `None` if the ray
    /// does not intersect with the plane.
    pub fn distance_to_plane(&self, plane: Plane) -> Option<f32> {
        let denominator = plane.normal.dot(self.direction);

        if denominator.abs() <= EPSILON {
            if plane.distance_to_point(self.origin).abs() <= EPSILON {
                // line is coplanar
                return Some(0.0);
            }
            return None;
        }

        let distance = -(self.origin.dot(plane.normal) + plane.distance) / denominator;

        if distance >= 0.0 {
            Some(distance)
        } else {
            None
        }
    }

    /// Intersect this [Ray] with a [Plane], returning the intersection point or `None` if there is no intersection.
    pub fn intersect_plane(&self, plane: Plane) -> Option<Vec3> {
        self.distance_to_plane(plane).map(|t| self.at(t))
    }
}

/// A two dimensional surface that extends infinitely in 3d space, represented in
/// [Hessian normal form](https://mathworld.wolfram.com/HessianNormalForm.html) by
/// a unit length normal vector and a constant.
pub struct Plane {
    /// Unit length `Vec3` defining the normal of the plane
    normal: Vec3,

    /// Signed distance from the origin to the plane
    distance: f32,
}

impl Plane {
    pub fn from_normal_and_coplanar_point(normal: Vec3, point: Vec3) -> Self {
        Self {
            normal,
            distance: -point.dot(normal),
        }
    }

    /// Returns the signed distance from the given point to this plane.
    pub fn distance_to_point(&self, point: Vec3) -> f32 {
        self.normal.dot(point) + self.distance
    }
}
