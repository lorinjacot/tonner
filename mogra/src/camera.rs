use glam::Mat4;
use thiserror::Error;

/// In this projection mode, an object's size in the rendered image decreases as its distance from the camera increases, creating a sense of depth and perspective.
///
/// The visible portion of the scene in the rendered image is defined by a truncated pyramid called the perspective frustum. `vertical_fov` defines the angle between the top and bottom planes of the frustum, while `aspect_ratio` defines the ratio of the width to the height of the frustum (usually the width and height of the viewport). The `near` and `far` planes define the distances from the camera to the near and far clipping planes, respectively. If `far` is `None`, an infinite perspective projection is used, which means that objects can be rendered at any distance from the camera.
#[derive(Debug, Clone, PartialEq)]
pub struct PerspectiveProjection {
    /// The vertical field of view, i.e. the angle between the top and bottom planes of the frustum. Expressed in radians. Must be greater than `0.0` and less than `π`. Default to `50°`.
    ///
    /// # Examples
    /// ```
    /// # use mogra::camera::PerspectiveProjection;
    /// let proj = PerspectiveProjection::default();
    /// assert_eq!(proj.vertical_fov, 50.0f32.to_radians());
    /// ```
    pub vertical_fov: f32,

    /// The aspect ratio of the frustum, i.e. the ratio of the width to the height of the frustum. Must be greater than `0.0`. Default to `1.0`.
    ///
    /// # Examples
    /// ```
    /// # use mogra::camera::PerspectiveProjection;
    /// let proj = PerspectiveProjection::default();
    /// assert_eq!(proj.aspect_ratio, 1.0);
    /// ```
    pub aspect_ratio: f32,

    /// The near plane of the perspective frustum. Must be strictly greater than `0.0` and strictly smaller than `far`. Default to `0.1`.
    ///
    /// # Examples
    /// ```
    /// # use mogra::camera::PerspectiveProjection;
    /// let proj = PerspectiveProjection::default();
    /// assert_eq!(proj.near, 0.1);
    /// ```
    pub near: f32,

    /// The far plane of the perspective frustum. Must be strictly greater than `near`. If `None`, an infinite perspective projection is used. Default to `None`.
    ///
    /// # Examples
    /// ```
    /// # use mogra::camera::PerspectiveProjection;
    /// let proj = PerspectiveProjection::default();
    /// assert_eq!(proj.far, None);
    /// ```
    pub far: Option<f32>,
}

impl PerspectiveProjection {
    fn check(&self) -> Result<(), PerspectiveProjectionError> {
        if self.vertical_fov <= 0.0 || self.vertical_fov >= std::f32::consts::PI {
            return Err(PerspectiveProjectionError::InvalidVerticalFov);
        }
        if self.aspect_ratio <= 0.0 {
            return Err(PerspectiveProjectionError::InvalidAspectRatio);
        }
        if self.near <= 0.0 {
            return Err(PerspectiveProjectionError::InvalidNearPlane);
        }
        if let Some(far) = self.far {
            if far <= self.near {
                return Err(PerspectiveProjectionError::InvalidFarPlane);
            }
        }
        Ok(())
    }

    /// *might* panic if the projection is invalid
    fn matrix(&self) -> Mat4 {
        match self.far {
            Some(far) => Mat4::perspective_rh(self.vertical_fov, self.aspect_ratio, self.near, far),
            None => Mat4::perspective_infinite_rh(self.vertical_fov, self.aspect_ratio, self.near),
        }
    }
}

impl Default for PerspectiveProjection {
    fn default() -> Self {
        Self {
            vertical_fov: 50.0f32.to_radians(),
            aspect_ratio: 1.0,
            near: 0.1,
            far: None,
        }
    }
}

/// Errors that can occur when creating a camera with a perspective projection.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum PerspectiveProjectionError {
    /// The `vertical_fov` must be greater than `0.0` and less than `π`.
    #[error("The vertical field of view must be greater than 0 and less than π.")]
    InvalidVerticalFov,

    /// The `aspect_ratio` must be greater than `0.0`.
    #[error("The aspect ratio must be greater than 0.")]
    InvalidAspectRatio,

    /// The `near` plane must be strictly greater than `0.0`.
    #[error("The near plane must be strictly greater than 0.")]
    InvalidNearPlane,

    /// The `far` plane must be strictly greater than `near`.
    #[error("The far plane must be strictly greater than the near plane.")]
    InvalidFarPlane,
}

/// In this projection mode, an object's size in the rendered image stays constant regardless of its distance from the camera.
///
/// The visible portion of the scene in the rendered image is defined by a rectangular box called the orthographic frustum. The frustum is defined by six planes: `left`, `right`, `bottom`, `top`, `near`, and `far`.
#[derive(Debug, Clone, PartialEq)]
pub struct OrthographicProjection {
    /// The left plane of the orthographic frustum. Must be strictly smaller than `right`. Default to `-1.0`.
    ///
    /// # Examples
    /// ```
    /// # use mogra::camera::OrthographicProjection;
    /// let proj = OrthographicProjection::default();
    /// assert_eq!(proj.left, -1.0);
    /// ```
    pub left: f32,

    /// The right plane of the orthographic frustum. Must be strictly greater than `left`. Default to `1.0`.
    ///
    /// # Examples
    /// ```
    /// # use mogra::camera::OrthographicProjection;
    /// let proj = OrthographicProjection::default();
    /// assert_eq!(proj.right, 1.0);
    /// ```
    pub right: f32,

    /// The bottom plane of the orthographic frustum. Must be strictly smaller than `top`. Default to `-1.0`.
    ///
    /// # Examples
    /// ```
    /// # use mogra::camera::OrthographicProjection;
    /// let proj = OrthographicProjection::default();
    /// assert_eq!(proj.bottom, -1.0);
    /// ```
    pub bottom: f32,

    /// The top plane of the orthographic frustum. Must be strictly greater than `bottom`. Default to `1.0`.
    ///
    /// # Examples
    /// ```
    /// # use mogra::camera::OrthographicProjection;
    /// let proj = OrthographicProjection::default();
    /// assert_eq!(proj.top, 1.0);
    /// ```
    pub top: f32,

    /// The near plane of the orthographic frustum. Must be strictly greater than `far`. Default to `0.1`.
    ///
    /// # Examples
    /// ```
    /// # use mogra::camera::OrthographicProjection;
    /// let proj = OrthographicProjection::default();
    /// assert_eq!(proj.near, 0.1);
    /// ```
    pub near: f32,

    /// The far plane of the orthographic frustum. Must be strictly smaller than `near`. Default to `1000.0`.
    ///
    /// # Examples
    /// ```
    /// # use mogra::camera::OrthographicProjection;
    /// let proj = OrthographicProjection::default();
    /// assert_eq!(proj.far, 1000.0);
    /// ```
    pub far: f32,
}

impl OrthographicProjection {
    fn check(&self) -> Result<(), OrthographicProjectionError> {
        if self.left >= self.right {
            return Err(OrthographicProjectionError::InvalidLeftRight);
        }
        if self.bottom >= self.top {
            return Err(OrthographicProjectionError::InvalidBottomTop);
        }
        if self.near <= 0.0 {
            return Err(OrthographicProjectionError::InvalidNearPlane);
        }
        if self.near >= self.far {
            return Err(OrthographicProjectionError::InvalidNearFar);
        }
        Ok(())
    }

    /// *might* panic if the projection is invalid
    fn matrix(&self) -> Mat4 {
        Mat4::orthographic_rh(
            self.left,
            self.right,
            self.bottom,
            self.top,
            self.near,
            self.far,
        )
    }
}

impl Default for OrthographicProjection {
    fn default() -> Self {
        Self {
            left: -1.0,
            right: 1.0,
            bottom: -1.0,
            top: 1.0,
            near: 0.1,
            far: 1000.0,
        }
    }
}

/// Errors that can occur when creating a camera with an orthographic projection.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum OrthographicProjectionError {
    /// The `left` plane must be strictly smaller than the `right` plane.
    #[error("The left plane must be strictly smaller than the right plane.")]
    InvalidLeftRight,

    /// The `bottom` plane must be strictly smaller than the `top` plane.
    #[error("The bottom plane must be strictly smaller than the top plane.")]
    InvalidBottomTop,

    /// The `near` plane must be strictly greater than `0.0`.
    #[error("The near plane must be strictly greater than 0.")]
    InvalidNearPlane,

    /// The `far` plane must be strictly greater than the `near` plane.
    #[error("The far plane must be strictly greater than the near plane.")]
    InvalidNearFar,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Camera {
    projection: Mat4,
}

impl Camera {
    pub fn perspective(
        projection: PerspectiveProjection,
    ) -> Result<Self, PerspectiveProjectionError> {
        projection.check()?;
        let projection = projection.matrix();
        Ok(Camera { projection })
    }

    pub fn orthographic(
        projection: OrthographicProjection,
    ) -> Result<Self, OrthographicProjectionError> {
        projection.check()?;
        let projection = projection.matrix();
        Ok(Camera { projection })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perspective_projection_checks() {
        let proj = PerspectiveProjection::default();
        assert_eq!(proj.check(), Ok(()));

        let proj = PerspectiveProjection {
            vertical_fov: 0.0,
            ..Default::default()
        };
        assert_eq!(
            proj.check(),
            Err(PerspectiveProjectionError::InvalidVerticalFov)
        );

        let proj = PerspectiveProjection {
            aspect_ratio: 0.0,
            ..Default::default()
        };
        assert_eq!(
            proj.check(),
            Err(PerspectiveProjectionError::InvalidAspectRatio)
        );

        let proj = PerspectiveProjection {
            near: 0.0,
            ..Default::default()
        };
        assert_eq!(
            proj.check(),
            Err(PerspectiveProjectionError::InvalidNearPlane)
        );

        let proj = PerspectiveProjection {
            near: 1.0,
            far: Some(0.5),
            ..Default::default()
        };
        assert_eq!(
            proj.check(),
            Err(PerspectiveProjectionError::InvalidFarPlane)
        );
    }

    #[test]
    fn orthographic_projection_checks() {
        let proj = OrthographicProjection::default();
        assert_eq!(proj.check(), Ok(()));

        let proj = OrthographicProjection {
            left: 1.0,
            right: 0.0,
            ..Default::default()
        };

        assert_eq!(
            proj.check(),
            Err(OrthographicProjectionError::InvalidLeftRight)
        );

        let proj = OrthographicProjection {
            bottom: 1.0,
            top: 0.0,
            ..Default::default()
        };

        assert_eq!(
            proj.check(),
            Err(OrthographicProjectionError::InvalidBottomTop)
        );

        let proj = OrthographicProjection {
            near: 0.0,
            ..Default::default()
        };
        assert_eq!(
            proj.check(),
            Err(OrthographicProjectionError::InvalidNearPlane)
        );

        let proj = OrthographicProjection {
            near: 1.0,
            far: 0.0,
            ..Default::default()
        };
        assert_eq!(
            proj.check(),
            Err(OrthographicProjectionError::InvalidNearFar)
        );
    }
}
