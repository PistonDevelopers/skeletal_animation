use interpolation::{self, Spatial};
use math::*;

pub trait Transform: Copy {
    fn identity() -> Self;
    fn add(self, other: Self) -> Self;
    fn subtract(self, other: Self) -> Self;
    fn lerp(self, other: Self, parameter: f32) -> Self;
    fn to_matrix(self) -> Matrix4<f32>;
    fn from_matrix(Matrix4<f32>) -> Self;
}

/// Transformation represented by separate scaling, translation, and rotation factors.
#[derive(Debug, Copy, Clone)]
pub struct QVTransform
{
    /// Translation
    pub translation: Vector3<f32>,

    /// Uniform scale factor.
    pub scale: f32,

    /// Rotation
    pub rotation: Quaternion<f32>

}

impl Transform for QVTransform {

    fn identity() -> QVTransform {
        QVTransform {
            translation: [0.0, 0.0, 0.0],
            scale: 1.0,
            rotation: quaternion_id(),
        }
    }

    fn add(self, other: QVTransform) -> QVTransform {
        QVTransform {
            translation: vec3_add(self.translation, other.translation),
            scale: self.scale + other.scale,
            rotation: quaternion_mul(self.rotation, other.rotation),
        }
    }

    fn subtract(self, other: QVTransform) -> QVTransform {
        QVTransform {
            translation: vec3_sub(self.translation, other.translation),
            scale: self.scale - other.scale,
            rotation: quaternion_mul(self.rotation, quaternion_conj(other.rotation)),
        }
    }

    fn lerp(self, other: QVTransform, parameter: f32) -> QVTransform {
        QVTransform {
            translation: interpolation::lerp(&self.translation, &other.translation, &parameter),
            scale: interpolation::lerp(&self.scale, &other.scale, &parameter),
            rotation: lerp_quaternion(&self.rotation, &other.rotation, &parameter),
        }
    }

    fn to_matrix(self) -> Matrix4<f32> {
        let mut m = quaternion_to_matrix(self.rotation);

        m[0][3] = self.translation[0];
        m[1][3] = self.translation[1];
        m[2][3] = self.translation[2];

        m
    }

    fn from_matrix(m: Matrix4<f32>) -> QVTransform {

        let rotation = matrix_to_quaternion(&m);

        let translation = [m[0][3],
                           m[1][3],
                           m[2][3]];

        QVTransform {
            rotation: rotation,
            scale: 1.0,
            translation: translation,
        }
    }

}

impl Transform for DualQuaternion<f32> {

    fn identity() -> DualQuaternion<f32> {
        dual_quaternion::id()
    }

    fn add(self, other: DualQuaternion<f32>) -> DualQuaternion<f32> {
        dual_quaternion::mul(self, other)
    }

    fn subtract(self, other: DualQuaternion<f32>) -> DualQuaternion<f32> {
        dual_quaternion::mul(self, dual_quaternion::conj(other))
    }

    fn lerp(self, other: DualQuaternion<f32>, parameter: f32) -> DualQuaternion<f32> {
        lerp_dual_quaternion(self, other, parameter)
    }

    fn to_matrix(self) -> Matrix4<f32> {

        let rotation = dual_quaternion::get_rotation(self);
        let translation = dual_quaternion::get_translation(self);

        let mut m = quaternion_to_matrix(rotation);

        m[0][3] = translation[0];
        m[1][3] = translation[1];
        m[2][3] = translation[2];

        m
    }

    fn from_matrix(m: Matrix4<f32>) -> DualQuaternion<f32> {

        let rotation = matrix_to_quaternion(&m);

        let translation = [m[0][3],
                           m[1][3],
                           m[2][3]];

        dual_quaternion::from_rotation_and_translation(rotation, translation)

    }
}

