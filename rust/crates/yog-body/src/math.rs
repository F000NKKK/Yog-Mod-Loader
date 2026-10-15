//! The little linear algebra bodies need: vectors and 4×4 matrices
//! (column-major, as JOML and the hosts' `PoseStack` keep them).

use std::ops::{Add, Mul, Neg, Sub};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);
    pub const ONE: Self = Self::new(1.0, 1.0, 1.0);

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn from_array(a: [f32; 3]) -> Self {
        Self::new(a[0], a[1], a[2])
    }

    pub fn to_array(self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }

    pub fn dot(self, o: Self) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub fn cross(self, o: Self) -> Self {
        Self::new(self.y * o.z - self.z * o.y, self.z * o.x - self.x * o.z, self.x * o.y - self.y * o.x)
    }

    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    /// This one scaled to length 1 (zero stays zero).
    pub fn normalize(self) -> Self {
        let l = self.length();
        if l < 1e-6 { self } else { self * (1.0 / l) }
    }

    pub fn lerp(self, to: Self, t: f32) -> Self {
        self + (to - self) * t
    }

    /// Component-wise product.
    pub fn scale(self, o: Self) -> Self {
        Self::new(self.x * o.x, self.y * o.y, self.z * o.z)
    }
}

impl Add for Vec3 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl Sub for Vec3 {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, k: f32) -> Self {
        Self::new(self.x * k, self.y * k, self.z * k)
    }
}

impl Neg for Vec3 {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y, -self.z)
    }
}

/// A 4×4 matrix, column-major: `m[col * 4 + row]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat4(pub [f32; 16]);

impl Default for Mat4 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Mat4 {
    pub const IDENTITY: Self = Self([1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]);

    fn at(&self, row: usize, col: usize) -> f32 {
        self.0[col * 4 + row]
    }

    pub fn translation(v: Vec3) -> Self {
        let mut m = Self::IDENTITY;
        m.0[12] = v.x;
        m.0[13] = v.y;
        m.0[14] = v.z;
        m
    }

    pub fn scaling(v: Vec3) -> Self {
        let mut m = Self::IDENTITY;
        m.0[0] = v.x;
        m.0[5] = v.y;
        m.0[10] = v.z;
        m
    }

    /// Turned `a` radians about x (right-handed, as `Axis.XP`).
    pub fn rotation_x(a: f32) -> Self {
        let (s, c) = a.sin_cos();
        Self([1.0, 0.0, 0.0, 0.0, 0.0, c, s, 0.0, 0.0, -s, c, 0.0, 0.0, 0.0, 0.0, 1.0])
    }

    pub fn rotation_y(a: f32) -> Self {
        let (s, c) = a.sin_cos();
        Self([c, 0.0, -s, 0.0, 0.0, 1.0, 0.0, 0.0, s, 0.0, c, 0.0, 0.0, 0.0, 0.0, 1.0])
    }

    pub fn rotation_z(a: f32) -> Self {
        let (s, c) = a.sin_cos();
        Self([c, s, 0.0, 0.0, -s, c, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0])
    }

    /// `ModelPart`'s turn: JOML `rotationZYX(z, y, x)` — z, then y, then x
    /// applied to the vector last.
    pub fn rotation_zyx(r: Vec3) -> Self {
        Self::rotation_z(r.z) * Self::rotation_y(r.y) * Self::rotation_x(r.x)
    }

    /// `ModelPart.translateAndRotate`: to the pivot, turned, scaled.
    pub fn part(pivot: Vec3, rotation: Vec3, scale: Vec3) -> Self {
        Self::translation(pivot) * Self::rotation_zyx(rotation) * Self::scaling(scale)
    }

    pub fn transform_point(&self, p: Vec3) -> Vec3 {
        Vec3::new(
            self.at(0, 0) * p.x + self.at(0, 1) * p.y + self.at(0, 2) * p.z + self.at(0, 3),
            self.at(1, 0) * p.x + self.at(1, 1) * p.y + self.at(1, 2) * p.z + self.at(1, 3),
            self.at(2, 0) * p.x + self.at(2, 1) * p.y + self.at(2, 2) * p.z + self.at(2, 3),
        )
    }

    /// A direction: turned and scaled, not moved.
    pub fn transform_dir(&self, d: Vec3) -> Vec3 {
        Vec3::new(
            self.at(0, 0) * d.x + self.at(0, 1) * d.y + self.at(0, 2) * d.z,
            self.at(1, 0) * d.x + self.at(1, 1) * d.y + self.at(1, 2) * d.z,
            self.at(2, 0) * d.x + self.at(2, 1) * d.y + self.at(2, 2) * d.z,
        )
    }

    /// A normal, kept a unit vector (good for rotations and uniform scales,
    /// which is what bodies use).
    pub fn transform_normal(&self, n: Vec3) -> Vec3 {
        self.transform_dir(n).normalize()
    }
}

impl Mul for Mat4 {
    type Output = Self;
    fn mul(self, o: Self) -> Self {
        let mut r = [0.0; 16];
        for col in 0..4 {
            for row in 0..4 {
                r[col * 4 + row] = (0..4).map(|k| self.at(row, k) * o.at(k, col)).sum();
            }
        }
        Self(r)
    }
}

/// Degrees to radians, for the formats (which are in degrees).
pub fn rad(deg: Vec3) -> Vec3 {
    deg * (std::f32::consts::PI / 180.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-5
    }

    #[test]
    fn rotations_are_right_handed() {
        let half = std::f32::consts::FRAC_PI_2;
        assert!(close(Mat4::rotation_x(half).transform_point(Vec3::new(0.0, 1.0, 0.0)), Vec3::new(0.0, 0.0, 1.0)));
        assert!(close(Mat4::rotation_y(half).transform_point(Vec3::new(0.0, 0.0, 1.0)), Vec3::new(1.0, 0.0, 0.0)));
        assert!(close(Mat4::rotation_z(half).transform_point(Vec3::new(1.0, 0.0, 0.0)), Vec3::new(0.0, 1.0, 0.0)));
    }

    #[test]
    fn a_part_moves_after_turning() {
        let m = Mat4::part(Vec3::new(5.0, 0.0, 0.0), Vec3::new(0.0, 0.0, std::f32::consts::FRAC_PI_2), Vec3::ONE);
        assert!(close(m.transform_point(Vec3::new(1.0, 0.0, 0.0)), Vec3::new(5.0, 1.0, 0.0)));
    }
}
