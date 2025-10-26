use wasm_bindgen::prelude::*;

use crate::wrapper;

wrapper!(
    "A 3-dimensional vector.",
    Vec3,
    glam::Vec3,
    fields: [
        x: f32,
        y: f32,
        z: f32
    ],
    consts: [
        "All zeroes."
        ZERO,

        "All ones."
        ONE,

        "All negative ones."
        NEG_ONE,

        "All MIN."
        MIN,

        "All MAX."
        MAX,

        "All NAN."
        NAN,

        "All INFINITY."
        INFINITY,

        "All NEG_INFINITY."
        NEG_INFINITY,

        "A unit vector pointing along the positive X axis."
        X,

        "A unit vector pointing along the positive Y axis."
        Y,

        "A unit vector pointing along the positive Z axis."
        Z,

        "A unit vector pointing along the negative X axis."
        NEG_X,

        "A unit vector pointing along the negative Y axis."
        NEG_Y,

        "A unit vector pointing along the negative Z axis."
        NEG_Z
    ]
);

#[wasm_bindgen]
impl Vec3 {
    /// Creates a new vector.
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self(glam::Vec3::new(x, y, z))
    }

    /// Creates a vector with all elements set to v.
    pub fn splat(v: f32) -> Self {
        Self(glam::Vec3::splat(v))
    }

    /// Creates a vector from the first 3 values in slice.
    ///
    /// # Panics
    /// Panics if slice is less than 3 elements long.
    pub fn from_slice(slice: &[f32]) -> Self {
        Self(glam::Vec3::from_slice(slice))
    }

    /// Writes the elements of `this` to the first 3 elements in slice.
    ///
    /// # Panics
    /// Panics if slice is less than 3 elements long.
    pub fn write_to_slice(self, slice: &mut [f32]) {
        self.0.write_to_slice(slice);
    }

    /// Creates a 3D vector from `this` with the given value of x.
    pub fn with_x(self, x: f32) -> Self {
        Self(self.0.with_x(x))
    }

    /// Creates a 3D vector from `this` with the given value of y.
    pub fn with_y(self, y: f32) -> Self {
        Self(self.0.with_y(y))
    }

    /// Creates a 3D vector from `this` with the given value of z.
    pub fn with_z(self, z: f32) -> Self {
        Self(self.0.with_z(z))
    }

    pub fn dot(self, rhs: Self) -> f32 {
        self.0.dot(rhs.0)
    }

    /// Computes the cross product of `this` and rhs.
    pub fn dot_into_vec(self, rhs: Self) -> Self {
        Self(self.0.dot_into_vec(rhs.0))
    }

    /// Returns a vector containing the minimum values for each element of `this` and rhs.
    ///
    /// In other words this computes `[min(this.x, rhs.x), min(this.y, rhs.y), ..]`.
    ///
    /// NaN propogation does not follow IEEE 754-2008 semantics for minNum and may differ on different SIMD architectures.
    pub fn min(self, rhs: Self) -> Self {
        Self(self.0.min(rhs.0))
    }

    /// Returns a vector containing the maximum values for each element of `this` and rhs.
    ///
    /// In other words this computes `[max(this.x, rhs.x), max(this.y, rhs.y), ..]`.
    ///
    /// NaN propogation does not follow IEEE 754-2008 semantics for maxNum and may differ on different SIMD architectures.
    pub fn max(self, rhs: Self) -> Self {
        Self(self.0.max(rhs.0))
    }

    /// Component-wise clamping of values.
    ///
    /// Each element in min must be less-or-equal to the corresponding element in max.
    ///
    /// NaN propogation does not follow IEEE 754-2008 semantics and may differ on different SIMD architectures.
    pub fn clamp(self, min: Self, max: Self) -> Self {
        Self(self.0.clamp(min.0, max.0))
    }

    /// Returns the horizontal minimum of `this`.
    ///
    /// In other words this computes `min(x, y, ..)`.
    ///
    /// NaN propogation does not follow IEEE 754-2008 semantics and may differ on different SIMD architectures.
    pub fn min_element(self) -> f32 {
        self.0.min_element()
    }

    /// Returns the horizontal maximum of `this`.
    ///
    /// In other words this computes `max(x, y, ..)`.
    ///
    /// NaN propogation does not follow IEEE 754-2008 semantics and may differ on different SIMD architectures.
    pub fn max_element(self) -> f32 {
        self.0.max_element()
    }

    /// Returns the index of the first minimum element of `this`.
    pub fn min_position(self) -> usize {
        self.0.min_position()
    }

    /// Returns the index of the first maximum element of `this`.
    pub fn max_position(self) -> usize {
        self.0.max_position()
    }

    /// Returns the sum of all elements of `this`.
    ///
    /// In other words, this computes `this.x + this.y + ..`.
    pub fn element_sum(self) -> f32 {
        self.0.element_sum()
    }

    /// Returns the product of all elements of `this`.
    ///
    /// In other words, this computes `this.x * this.y * ..`.
    pub fn element_product(self) -> f32 {
        self.0.element_product()
    }

    /// Returns a vector containing the absolute value of each element of `this`.
    pub fn abs(self) -> Self {
        Self(self.0.abs())
    }

    /// Returns a vector with elements representing the sign of `this`.
    /// - `1.0` if the number is positive, `+0.0` or `INFINITY`
    /// - `-1.0` if the number is negative, `-0.0` or `NEG_INFINITY`
    /// - `NAN` if the number is `NAN`
    pub fn signum(self) -> Self {
        Self(self.0.signum())
    }

    /// Returns a vector with signs of `rhs` and the magnitudes of `this`.
    pub fn copysign(self, rhs: Self) -> Self {
        Self(self.0.copysign(rhs.0))
    }

    /// Returns a bitmask with the lowest 3 bits set to the sign bits from the elements of `this`.
    /// 
    /// A negative element results in a `1` bit and a positive element in a `0` bit. Element `x` goes
    /// into the first lowest bit, element `y` into the second, etc.
    /// 
    /// An element is negative if it has a negative sign, including -0.0, NaNs with negative sign bit and negative infinity.
    pub fn is_negative_bitmask(self) -> u32 {
        self.0.is_negative_bitmask()
    }

    /// Returns true if, and only if, all elements are finite. If any element is either NaN, positive or negative infinity, this will return false.
    pub fn is_finite(self) -> bool {
        self.0.is_finite()
    }

    /// Returns true if any elements are NaN.
    pub fn is_nan(self) -> bool {
        self.0.is_nan()
    }

    /// Computes the length of `this`.
    pub fn length(self) -> f32 {
        self.0.length()
    }

    /// Computes the squared length of self.
    /// 
    /// This is faster than `length()` as it avoids a square root operation.
    pub fn length_squared(self) -> f32 {
        self.0.length_squared()
    }

    /// Computes `1.0 / length()`.
    /// 
    /// For valid results, `this` must not be of length zero.
    pub fn length_recip(self) -> f32 {
        self.0.length_recip()
    }

    /// Computes the Euclidean distance between two points in space.
    pub fn distance(self, rhs: Self) -> f32 {
        self.0.distance(rhs.0)
    }

    /// Compute the squared euclidean distance between two points in space.
    pub fn distance_squared(self, rhs: Self) -> f32 {
        self.0.distance_squared(rhs.0)
    }
}
