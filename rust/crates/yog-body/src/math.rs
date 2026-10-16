//! The vectors and matrices bodies use are `yog-graphics`' own.

pub use yog_graphics::{Mat4, Vec3, rad};

/// `ModelPart.translateAndRotate`: to the pivot, turned, scaled.
pub fn part(pivot: Vec3, rotation: Vec3, scale: Vec3) -> Mat4 {
    Mat4::translation(pivot) * Mat4::rotation_zyx(rotation) * Mat4::scaling(scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_part_moves_after_turning() {
        let m = part(Vec3::new(5.0, 0.0, 0.0), Vec3::new(0.0, 0.0, std::f32::consts::FRAC_PI_2), Vec3::ONE);
        let p = m.transform_point(Vec3::new(1.0, 0.0, 0.0));
        assert!((p.x - 5.0).abs() < 1e-5 && (p.y - 1.0).abs() < 1e-5, "{p:?}");
    }
}
