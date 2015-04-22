use interpolation::{self, Spatial};
use math::*;

/// Transformation represented by separate scaling, translation, and rotation factors.
#[derive(Debug, Copy, Clone)]
pub struct Transform
{
    /// Translation
    pub translation: Vector3<f32>,

    /// Uniform scale factor.
    pub scale: f32,

    /// Rotation
    pub rotation: Quaternion<f32>

}

impl Transform {

    pub fn identity() -> Transform {
        Transform {
            translation: [0.0, 0.0, 0.0],
            scale: 1.0,
            rotation: quaternion_id(),
        }
    }

    pub fn add(self, other: Transform) -> Transform {
        Transform {
            translation: vec3_add(self.translation, other.translation),
            scale: self.scale + other.scale,
            rotation: quaternion_mul(self.rotation, other.rotation),
        }
    }

    pub fn subtract(self, other: Transform) -> Transform {
        Transform {
            translation: vec3_sub(self.translation, other.translation),
            scale: self.scale - other.scale,
            rotation: quaternion_mul(self.rotation, quaternion_conj(other.rotation)),
        }
    }

    pub fn lerp(self, other: Transform, parameter: f32) -> Transform {
        Transform {
            translation: interpolation::lerp(&self.translation, &other.translation, &parameter),
            scale: interpolation::lerp(&self.scale, &other.scale, &parameter),
            rotation: lerp_quaternion(&self.rotation, &other.rotation, &parameter),
        }
    }
}
