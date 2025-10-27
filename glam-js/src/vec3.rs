use glam::Vec3Swizzles;
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

    /// Returns the element-wise quotient of [Euclidean division] of `this` by `rhs`.
    pub fn div_euclid(self, rhs: Self) -> Self {
        Self(self.0.div_euclid(rhs.0))
    }

    /// Returns the element-wise remainder of Euclidean division of `self` by `rhs`.
    pub fn rem_euclid(self, rhs: Self) -> Self {
        Self(self.0.rem_euclid(rhs.0))
    }

    /// Returns self normalized to length 1.0.
    ///
    /// For valid results, `this` must be finite and not of length zero, nor very close to zero.
    ///
    /// See also {@link Vec3::try_normalize()} and {@link Vec3::normalize_or_zero()}.
    pub fn normalize(self) -> Self {
        Self(self.0.normalize())
    }

    /// Returns `this` normalized to length 1.0 if possible, else returns None.
    ///
    /// In particular, if the input is zero (or very close to zero), or non-finite, the result of this operation will be None.
    ///
    /// See also {@link Vec::normalize_or_zero()}.
    pub fn try_normalize(self) -> Option<Self> {
        self.0.try_normalize().map(Self)
    }

    /// Returns `this` normalized to length 1.0 if possible, else returns a fallback value.
    ///
    /// In particular, if the input is zero (or very close to zero), or non-finite, the result of this operation will be the fallback value.
    ///
    /// See also {@link Vec3::try_normalize()}.
    pub fn normalize_or(self, fallback: Self) -> Self {
        Self(self.0.normalize_or(fallback.0))
    }

    /// Returns `this` normalized to length 1.0 if possible, else returns zero.
    ///
    /// In particular, if the input is zero (or very close to zero), or non-finite, the result of this operation will be zero.
    ///
    /// See also {@link Vec3::try_normalize()}.
    pub fn normalize_or_zero(self) -> Self {
        Self(self.0.normalize_or_zero())
    }

    /// Returns whether `this` is length `1.0` or not.
    ///
    /// Uses a precision threshold of approximately `1e-4`.
    pub fn is_normalized(self) -> bool {
        self.0.is_normalized()
    }

    /// Returns the vector projection of `this` onto `rhs`.
    ///
    /// `rhs` must be of non-zero length.
    pub fn project_onto(self, rhs: Self) -> Self {
        Self(self.0.project_onto(rhs.0))
    }

    /// Returns the vector rejection of `this` from `rhs`.
    ///
    /// The vector rejection is the vector perpendicular to theprojection of `this` onto `rhs`,
    /// in rhs words the result of `this - this.project_onto(rhs)`.
    pub fn reject_from(self, rhs: Self) -> Self {
        Self(self.0.reject_from(rhs.0))
    }

    /// Returns the vector projection of `this` onto `rhs`.
    ///
    /// `rhs` must be normalized.
    pub fn project_onto_normalized(self, rhs: Self) -> Self {
        Self(self.0.project_onto_normalized(rhs.0))
    }

    /// Returns the vector rejection of `this` from `rhs`.
    ///
    /// The vector rejection is the vector perpendicular to the projection of `this` onto `rhs`,
    /// in rhs words the result of `this - this.project_onto(rhs)`.
    ///
    /// `rhs` must be normalized.
    pub fn reject_from_normalized(self, rhs: Self) -> Self {
        Self(self.0.reject_from_normalized(rhs.0))
    }

    /// Returns a vector containing the nearest integer to a number for each element of `this`.
    /// Round half-way cases away from 0.0.
    pub fn round(self) -> Self {
        Self(self.0.round())
    }

    /// Returns a vector containing the largest integer less than or equal to a number for each element of `this`.
    pub fn floor(self) -> Self {
        Self(self.0.floor())
    }

    /// Returns a vector containing the smallest integer greater than or equal to a number for each element of `this`.
    pub fn ceil(self) -> Self {
        Self(self.0.ceil())
    }

    /// Returns a vector containing the integer part each element of `this`. This means numbers are always truncated towards zero.
    pub fn trunc(self) -> Self {
        Self(self.0.trunc())
    }

    /// Returns a vector containing the fractional part of the vector as `this - this.trunc()`.
    ///
    /// Note that this differs from the GLSL implementation of `fract` which returns `this - this.floor()`.
    ///
    /// Note that this is fast but not precise for large numbers.
    pub fn fract(self) -> Self {
        Self(self.0.fract())
    }

    /// Returns a vector containing the fractional part of the vector as `this - this.floor()`.
    ///
    /// Note that this differs from the Rust implementation of `fract` which returns `this - this.trunc()`.
    ///
    /// Note that this is fast but not precise for large numbers.
    pub fn fract_gl(self) -> Self {
        Self(self.0.fract_gl())
    }

    /// Returns a vector containing `e^this` (the exponential function) for each element of `this`.
    pub fn exp(self) -> Self {
        Self(self.0.exp())
    }

    /// Returns a vector containing each element of `this` raised to the power of `n`.
    pub fn powf(self, n: f32) -> Self {
        Self(self.0.powf(n))
    }

    /// Returns a vector containing the reciprocal `1.0/n` of each element of `this`.
    pub fn recip(self) -> Self {
        Self(self.0.recip())
    }

    /// Performs a linear interpolation between `this` and `rhs` based on the value `s`.
    ///
    /// When `s` is `0.0`, the result will be equal to `this`. When `s` is `1.0`, the
    /// result will be equal to `rhs`. When `s` is outside of range `[0, 1]`, the result
    /// is linearly extrapolated.
    pub fn lerp(self, rhs: Self, s: f32) -> Self {
        Self(self.0.lerp(rhs.0, s))
    }

    /// Moves towards `rhs` based on the value `d`.
    ///
    /// When `d` is `0.0`, the result will be equal to `this`. When `d` is equal to
    /// `this.distance(rhs)`, the result will be equal to `rhs`. Will not go past `rhs`.
    pub fn move_towards(&self, rhs: Self, d: f32) -> Self {
        Self(self.0.move_towards(rhs.0, d))
    }

    /// Calculates the midpoint between `this` and `rhs`.
    ///
    /// The midpoint is the average of, or halfway point between, two vectors.
    /// `a.midpoint(b)` should yield the same result as `a.lerp(b, 0.5)` while being slightly
    /// cheaper to compute.
    pub fn midpoint(self, rhs: Self) -> Self {
        Self(self.0.midpoint(rhs.0))
    }

    /// Returns true if the absolute difference of all elements between `this` and `rhs` is less
    /// than or equal to `max_abs_diff`.
    ///
    /// This can be used to compare if two vectors contain similar elements. It works best when
    /// comparing with a known value. The `max_abs_diff` that should be used used depends on the
    /// values being compared against.
    pub fn abs_diff_eq(self, rhs: Self, max_abs_diff: f32) -> bool {
        self.0.abs_diff_eq(rhs.0, max_abs_diff)
    }

    /// Returns a vector with a length no less than `min` and no more than `max`.
    pub fn clamp_length(self, min: f32, max: f32) -> Self {
        Self(self.0.clamp_length(min, max))
    }

    /// Returns a vector with a length no more than `max`.
    pub fn clamp_length_max(self, max: f32) -> Self {
        Self(self.0.clamp_length_max(max))
    }

    /// Returns a vector with a length no less than `min`.
    pub fn clamp_length_min(self, min: f32) -> Self {
        Self(self.0.clamp_length_min(min))
    }

    /// Fused multiply-add. Computes `(this * a) + b` element-wise with only one
    /// rounding error, yielding a more accurate result than an unfused multiply-add.
    ///
    /// Using `mul_add` may be more performant than an unfused multiply-add if the target
    /// architecture has a dedicated fma CPU instruction. However, this is not always true,
    /// and will be heavily dependant on designing algorithms with specific target hardware
    /// in mind.
    pub fn mul_add(self, a: Self, b: Self) -> Self {
        Self(self.0.mul_add(a.0, b.0))
    }

    /// Returns the reflection vector for a given incident vector `this` and surface normal `normal`.
    ///
    /// `normal` must be normalized.
    pub fn reflect(self, normal: Self) -> Self {
        Self(self.0.reflect(normal.0))
    }

    /// Returns the refraction direction for a given incident vector `this`, surface normal `normal` and
    /// ratio of indices of refraction, `eta`. When total internal reflection occurs, a zero vector will
    /// be returned.
    ///
    /// `this` and `normal` must be normalized.
    pub fn refract(self, normal: Self, eta: f32) -> Self {
        Self(self.0.refract(normal.0, eta))
    }

    /// Returns the angle (in radians) between two vectors in the range `[0, +π]`.
    ///
    /// The inputs do not need to be unit vectors however they must be non-zero.
    pub fn angle_between(self, rhs: Self) -> f32 {
        self.0.angle_between(rhs.0)
    }

    /// Rotates towards `rhs` up to `max_angle` (in radians).
    ///
    /// When `max_angle` is `0.0`, the result will be equal to `this`. When `max_angle` is equal to
    /// `this.angle_between(rhs)`, the result will be parallel to `rhs`. If `max_angle` is negative,
    /// rotates towards the exact opposite of `rhs`. Will not go past the target.
    pub fn rotate_towards(self, rhs: Self, max_angle: f32) -> Self {
        Self(self.0.rotate_towards(rhs.0, max_angle))
    }

    /// Returns some vector that is orthogonal to the given one.
    ///
    /// The input vector must be finite and non-zero.
    ///
    /// The output vector is not necessarily unit length. For that use
    /// {@link Vec.any_orthonormal_vector()} instead.
    pub fn any_orthogonal_vector(&self) -> Self {
        Self(self.0.any_orthogonal_vector())
    }

    /// Returns any unit vector that is orthogonal to the given one.
    ///
    /// The input vector must be unit length.
    pub fn any_orthonormal_vector(&self) -> Self {
        Self(self.0.any_orthonormal_vector())
    }

    /// Performs a spherical linear interpolation between `this` and `rhs` based on the value `s`.
    ///
    /// When `s` is `0.0`, the result will be equal to `this`. When `s` is `1.0`, the result will
    /// be equal to `rhs`. When `s` is outside of range `[0, 1]`, the result is linearly extrapolated.
    pub fn slerp(self, rhs: Self, s: f32) -> Self {
        Self(self.0.slerp(rhs.0, s))
    }

    /// Returns `this + rhs` component-wise.
    pub fn add(&self, rhs: &Self) -> Self {
        Self(self.0 + rhs.0)
    }

    /// Returns `this + Vec3.splat(rhs)`.
    pub fn add_float(&self, rhs: f32) -> Self {
        Self(self.0 + rhs)
    }

    /// Performs `this += rhs` component-wise.
    pub fn add_assign(&mut self, rhs: &Self) {
        self.0 += rhs.0;
    }

    /// Performs `this += Vec3.splat(rhs)`.
    pub fn add_assign_float(&mut self, rhs: f32) {
        self.0 += rhs;
    }

    /// Returns a duplicate of `this`.
    pub fn clone(&self) -> Self {
        Self(self.0.clone())
    }

    /// Returns `this / rhs` component-wise.
    pub fn div(&self, rhs: &Self) -> Self {
        Self(self.0 / rhs.0)
    }

    /// Returns `this / Vec3.splat(rhs)`.
    pub fn div_float(&self, rhs: f32) -> Self {
        Self(self.0 / rhs)
    }

    /// Performs `this /= rhs` component-wise.
    pub fn div_assign(&mut self, rhs: &Self) {
        self.0 /= rhs.0;
    }

    /// Performs `this /= Vec3.splat(rhs)`.
    pub fn div_assign_float(&mut self, rhs: f32) {
        self.0 /= rhs;
    }

    /// Returns the component corresponding to `index`.
    ///
    /// For example, `this.index(0)` would return `this.x`.
    pub fn index(&self, index: usize) -> f32 {
        self.0[index]
    }

    /// Set the component corresponding to `index` to `value`.
    ///
    /// For example, `this.set_index(0, 0.5)` is the same as `this.x = 0.5`.
    pub fn set_index(&mut self, index: usize, value: f32) {
        self.0[index] = value
    }

    /// Returns `this * rhs` component-wise.
    pub fn mul(&self, rhs: &Self) -> Self {
        Self(self.0 * rhs.0)
    }

    /// Returns `this * Vec3.splat(rhs)`.
    pub fn mul_float(&self, rhs: f32) -> Self {
        Self(self.0 * rhs)
    }

    /// Performs `this *= rhs` component-wise.
    pub fn mul_assign(&mut self, rhs: &Self) {
        self.0 *= rhs.0;
    }

    /// Performs `this *= Vec3.splat(rhs)`.
    pub fn mul_assign_float(&mut self, rhs: f32) {
        self.0 *= rhs;
    }

    /// Returns `-this` component-wise.
    pub fn neg(&self) -> Self {
        Self(-self.0)
    }

    /// Returns `this == other`.
    ///
    /// In other words, returns true if both vectors have the same components.
    pub fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }

    /// Returns `this != other`.
    ///
    /// In other words, returns true if any component is different.
    pub fn ne(&self, other: &Self) -> bool {
        self.0 != other.0
    }

    /// Returns `this % rhs` component-wise.
    pub fn rem(&self, rhs: &Self) -> Self {
        Self(self.0 % rhs.0)
    }

    /// Returns `this % Vec3.splat(rhs)`.
    pub fn rem_float(&self, rhs: f32) -> Self {
        Self(self.0 % rhs)
    }

    /// Performs `this %= rhs` component-wise.
    pub fn rem_assign(&mut self, rhs: &Self) {
        self.0 %= rhs.0;
    }

    /// Performs `this %= Vec3.splat(rhs)`.
    pub fn rem_assign_float(&mut self, rhs: f32) {
        self.0 %= rhs;
    }

    /// Returns `this - rhs` component-wise.
    pub fn sub(&self, rhs: &Self) -> Self {
        Self(self.0 - rhs.0)
    }

    /// Returns `this - Vec3.splat(rhs)`.
    pub fn sub_float(&self, rhs: f32) -> Self {
        Self(self.0 - rhs)
    }

    /// Performs `this -= rhs` component-wise.
    pub fn sub_assign(&mut self, rhs: &Self) {
        self.0 -= rhs.0;
    }

    /// Performs `this -= Vec3.splat(rhs)`.
    pub fn sub_assign_float(&mut self, rhs: f32) {
        self.0 -= rhs;
    }

    #[wasm_bindgen(getter)]
    pub fn xxx(self) -> Self {
        Self(self.0.xxx())
    }

    #[wasm_bindgen(getter)]
    pub fn xxy(self) -> Self {
        Self(self.0.xxy())
    }

    #[wasm_bindgen(getter)]
    pub fn xxz(self) -> Self {
        Self(self.0.xxz())
    }

    #[wasm_bindgen(getter)]
    pub fn xyx(self) -> Self {
        Self(self.0.xyx())
    }

    #[wasm_bindgen(getter)]
    pub fn xyy(self) -> Self {
        Self(self.0.xyy())
    }

    #[wasm_bindgen(getter)]
    pub fn xyz(self) -> Self {
        Self(self.0.xyz())
    }

    #[wasm_bindgen(getter)]
    pub fn xzx(self) -> Self {
        Self(self.0.xzx())
    }

    #[wasm_bindgen(getter)]
    pub fn xzy(self) -> Self {
        Self(self.0.xzy())
    }

    #[wasm_bindgen(getter)]
    pub fn xzz(self) -> Self {
        Self(self.0.xzz())
    }

    #[wasm_bindgen(getter)]
    pub fn yxx(self) -> Self {
        Self(self.0.yxx())
    }

    #[wasm_bindgen(getter)]
    pub fn yxy(self) -> Self {
        Self(self.0.yxy())
    }

    #[wasm_bindgen(getter)]
    pub fn yxz(self) -> Self {
        Self(self.0.yxz())
    }

    #[wasm_bindgen(getter)]
    pub fn yyx(self) -> Self {
        Self(self.0.yyx())
    }

    #[wasm_bindgen(getter)]
    pub fn yyy(self) -> Self {
        Self(self.0.yyy())
    }

    #[wasm_bindgen(getter)]
    pub fn yyz(self) -> Self {
        Self(self.0.yyz())
    }

    #[wasm_bindgen(getter)]
    pub fn yzx(self) -> Self {
        Self(self.0.yzx())
    }

    #[wasm_bindgen(getter)]
    pub fn yzy(self) -> Self {
        Self(self.0.yzy())
    }

    #[wasm_bindgen(getter)]
    pub fn yzz(self) -> Self {
        Self(self.0.yzz())
    }

    #[wasm_bindgen(getter)]
    pub fn zxx(self) -> Self {
        Self(self.0.zxx())
    }

    #[wasm_bindgen(getter)]
    pub fn zxy(self) -> Self {
        Self(self.0.zxy())
    }

    #[wasm_bindgen(getter)]
    pub fn zxz(self) -> Self {
        Self(self.0.zxz())
    }

    #[wasm_bindgen(getter)]
    pub fn zyx(self) -> Self {
        Self(self.0.zyx())
    }

    #[wasm_bindgen(getter)]
    pub fn zyy(self) -> Self {
        Self(self.0.zyy())
    }

    #[wasm_bindgen(getter)]
    pub fn zyz(self) -> Self {
        Self(self.0.zyz())
    }

    #[wasm_bindgen(getter)]
    pub fn zzx(self) -> Self {
        Self(self.0.zzx())
    }

    #[wasm_bindgen(getter)]
    pub fn zzy(self) -> Self {
        Self(self.0.zzy())
    }

    #[wasm_bindgen(getter)]
    pub fn zzz(self) -> Self {
        Self(self.0.zzz())
    }
}

impl From<glam::Vec3> for Vec3 {
    fn from(value: glam::Vec3) -> Self {
        Self(value)
    }
}

impl From<Vec3> for glam::Vec3 {
    fn from(value: Vec3) -> Self {
        value.0
    }
}
