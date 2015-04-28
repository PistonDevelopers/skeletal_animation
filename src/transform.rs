use interpolation::{self, Spatial};
use math::*;

pub trait Transform: Copy {
    fn identity() -> Self;
    fn concat(self, other: Self) -> Self;
    fn inverse(self) -> Self;
    fn lerp(self, other: Self, parameter: f32) -> Self;
    fn transform_vector(self, v: Vector3<f32>) -> Vector3<f32>;
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

    fn concat(self, other: QVTransform) -> QVTransform {
        QVTransform::from_matrix(self.to_matrix().concat(other.to_matrix()))
    }

    fn inverse(self) -> QVTransform {
        QVTransform::from_matrix(self.to_matrix().inverse())
    }

    fn lerp(self, other: QVTransform, parameter: f32) -> QVTransform {
        QVTransform {
            translation: interpolation::lerp(&self.translation, &other.translation, &parameter),
            scale: interpolation::lerp(&self.scale, &other.scale, &parameter),
            rotation: lerp_quaternion(&self.rotation, &other.rotation, &parameter),
        }
    }

    fn transform_vector(self, v: Vector3<f32>) -> Vector3<f32> {
        let v = quaternion::rotate_vector(self.rotation, v);
        let v = vec3_add(v, self.translation);
        vec3_scale(v, self.scale)
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

    fn concat(self, other: DualQuaternion<f32>) -> DualQuaternion<f32> {
        dual_quaternion::mul(self, other)
    }

    fn inverse(self) -> DualQuaternion<f32> {
        dual_quaternion::conj(self)
    }

    fn lerp(self, other: DualQuaternion<f32>, parameter: f32) -> DualQuaternion<f32> {
        lerp_dual_quaternion(self, other, parameter)
    }

    fn transform_vector(self, v: Vector3<f32>) -> Vector3<f32> {
        let t = dual_quaternion::get_translation(self);
        let r = dual_quaternion::get_rotation(self);
        vec3_add(quaternion::rotate_vector(r, v), t)
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

        let rotation = matrix_to_quaternion(&mat4_transposed(m));

        let translation = [m[0][3],
                           m[1][3],
                           m[2][3]];

        dual_quaternion::from_rotation_and_translation(rotation, translation)

    }
}

impl Transform for Matrix4<f32> {

    fn identity() -> Matrix4<f32> {
        mat4_id()
    }

    fn concat(self, other: Matrix4<f32>) -> Matrix4<f32> {
        row_mat4_mul(self, other)
    }

    fn inverse(self) -> Matrix4<f32> {
        mat4_inv(self)
    }

    fn lerp(self, other: Matrix4<f32>, parameter: f32) -> Matrix4<f32> {
        let q1 = DualQuaternion::from_matrix(self);
        let q2 = DualQuaternion::from_matrix(other);
        q1.lerp(q2, parameter).to_matrix()
    }

    fn transform_vector(self, v: Vector3<f32>) -> Vector3<f32> {
        let t = row_mat4_transform(self, [v[0], v[1], v[2], 1.0]);
        [t[0], t[1], t[2]]
    }

    fn to_matrix(self) -> Matrix4<f32> { self }

    fn from_matrix(m: Matrix4<f32>) -> Matrix4<f32> { m }
}
