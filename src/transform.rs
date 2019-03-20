use interpolation;
use math::*;

pub trait Transform: Copy {
    fn identity() -> Self;
    fn concat(self, other: Self) -> Self;
    fn inverse(self) -> Self;
    fn lerp(self, other: Self, parameter: f32) -> Self;
    fn transform_vector(self, v: Vector3<f32>) -> Vector3<f32>;
    fn to_matrix(self) -> Matrix4<f32>;
    fn from_matrix(Matrix4<f32>) -> Self;
    fn set_rotation(&mut self, rotation: Quaternion<f32>);
    fn get_rotation(self) -> Quaternion<f32>;
    fn set_translation(&mut self, translation: Vector3<f32>);
    fn get_translation(self) -> Vector3<f32>;
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

    fn identity() -> Self {
        Self {
            translation: [0.0, 0.0, 0.0],
            scale: 1.0,
            rotation: quaternion_id(),
        }
    }

    fn set_rotation(&mut self, rotation: Quaternion<f32>) {
        self.rotation = rotation;
    }

    fn get_rotation(self) -> Quaternion<f32> {
        self.rotation
    }

    fn set_translation(&mut self, translation: Vector3<f32>) {
        self.translation = translation;
    }

    fn get_translation(self) -> Vector3<f32> {
        self.translation
    }

    fn concat(self, other: Self) -> Self {
        Self::from_matrix(self.to_matrix().concat(other.to_matrix()))
    }

    fn inverse(self) -> Self {
        Self::from_matrix(self.to_matrix().inverse())
    }

    fn lerp(self, other: Self, parameter: f32) -> Self {
        Self {
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

    fn from_matrix(m: Matrix4<f32>) -> Self {

        let rotation = matrix_to_quaternion(&m);

        let translation = [m[0][3],
                           m[1][3],
                           m[2][3]];

        Self {
            rotation: rotation,
            scale: 1.0,
            translation: translation,
        }
    }

}

impl Transform for DualQuaternion<f32> {

    fn identity() -> Self {
        dual_quaternion::id()
    }

    fn set_rotation(&mut self, rotation: Quaternion<f32>) {
        let t = dual_quaternion::get_translation(*self);
        *self = dual_quaternion::from_rotation_and_translation(rotation, t);
    }

    fn get_rotation(self) -> Quaternion<f32> {
        dual_quaternion::get_rotation(self)
    }

    fn set_translation(&mut self, translation: Vector3<f32>) {
        let rotation = dual_quaternion::get_rotation(*self);
        *self = dual_quaternion::from_rotation_and_translation(rotation, translation);
    }

    fn get_translation(self) -> Vector3<f32> {
        dual_quaternion::get_translation(self)
    }

    fn concat(self, other: Self) -> Self {
        dual_quaternion::mul(self, other)
    }

    fn inverse(self) -> Self {
        dual_quaternion::conj(self)
    }

    fn lerp(self, other: Self, parameter: f32) -> Self {
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

        let mut m = mat4_transposed(quaternion_to_matrix(rotation));

        m[0][3] = translation[0];
        m[1][3] = translation[1];
        m[2][3] = translation[2];

        m
    }

    fn from_matrix(m: Matrix4<f32>) -> Self {
        let rotation = matrix_to_quaternion(&mat4_transposed(m));

        let translation = [m[0][3],
                           m[1][3],
                           m[2][3]];

        dual_quaternion::from_rotation_and_translation(rotation, translation)
    }
}

impl Transform for Matrix4<f32> {

    fn identity() -> Self {
        mat4_id()
    }

    fn set_rotation(&mut self, rotation: Quaternion<f32>) {

        let rotation = quaternion_to_matrix(rotation);

        self[0][0] = rotation[0][0];
        self[1][0] = rotation[1][0];
        self[2][0] = rotation[2][0];

        self[0][1] = rotation[0][1];
        self[1][1] = rotation[1][1];
        self[2][1] = rotation[2][1];

        self[0][2] = rotation[0][2];
        self[1][2] = rotation[1][2];
        self[2][2] = rotation[2][2];
    }

    fn get_rotation(self) -> Quaternion<f32> {
        matrix_to_quaternion(&self)
    }

    fn set_translation(&mut self, translation: Vector3<f32>) {
        self[0][3] = translation[0];
        self[1][3] = translation[1];
        self[2][3] = translation[2];
    }

    fn get_translation(self) -> Vector3<f32> {
        [self[0][3],
         self[1][3],
         self[2][3]]
    }

    fn concat(self, other: Self) -> Self {
        row_mat4_mul(self, other)
    }

    fn inverse(self) -> Self {
        mat4_inv(self)
    }

    fn lerp(self, other: Self, parameter: f32) -> Self {
        let q1 = DualQuaternion::from_matrix(self);
        let q2 = DualQuaternion::from_matrix(other);
        q1.lerp(q2, parameter).to_matrix()
    }

    fn transform_vector(self, v: Vector3<f32>) -> Vector3<f32> {
        let t = row_mat4_transform(self, [v[0], v[1], v[2], 1.0]);
        [t[0], t[1], t[2]]
    }

    fn to_matrix(self) -> Self { self }

    fn from_matrix(m: Self) -> Self { m }
}

pub trait FromTransform<T: Transform> {
    fn from_transform(t: T) -> Self;
}

impl FromTransform<DualQuaternion<f32>> for DualQuaternion<f32> {
    fn from_transform(t: Self) -> Self { t }
}

impl<T: Transform> FromTransform<T> for Matrix4<f32> {
    fn from_transform(t: T) -> Self {
        t.to_matrix()
    }
}

#[cfg(test)]
mod test {

    use vecmath;
    use quaternion;
    use dual_quaternion;

    use super::Transform;

    static EPSILON: f32 = 0.000001;

    #[test]
    fn test_dual_quaternion_to_matrix() {

        let a = [1.0, 0.0, 0.0];
        let b = [0.0, 1.0, 0.0];

        let q = quaternion::rotation_from_to(a, b);

        let dq = dual_quaternion::from_rotation_and_translation(q, [0.0, 0.0, 0.0]);
        println!("{:?}", dq.transform_vector(a));
        assert!(vecmath::vec3_len(vecmath::vec3_sub(b, dq.transform_vector(a))) < EPSILON);

        let m = dq.to_matrix();
        println!("{:?}", m.transform_vector(a));
        assert!(vecmath::vec3_len(vecmath::vec3_sub(b, m.transform_vector(a))) < EPSILON);
    }

    #[test]
    fn test_dual_quaternion_set_rotation() {

        let a = [1.0, 0.0, 0.0];
        let b = [0.0, 1.0, 0.0];

        let q = quaternion::rotation_from_to(a, b);
        let q2 = quaternion::rotation_from_to(b, a);

        let mut dq = dual_quaternion::from_rotation_and_translation(q, [0.0, 1.0, 0.0]);

        println!("{:?}", dq.transform_vector(a));
        assert!(vecmath::vec3_len(vecmath::vec3_sub([0.0, 2.0, 0.0],
                                                    dq.transform_vector(a))) < EPSILON);

        dq.set_rotation(q2);
        println!("{:?}", dq.transform_vector(b));
        assert!(vecmath::vec3_len(vecmath::vec3_sub([1.0, 1.0, 0.0],
                                                    dq.transform_vector(b))) < EPSILON);
    }
}
