use vecmath::Matrix4;
use quaternion::Quaternion;
use std::mem;

pub fn inv_sqrt(x: f32) -> f32 {

    let x2: f32 = x * 0.5;
    let mut y: f32 = x;

    let mut i: i32 = unsafe { mem::transmute(y) };
    i = 0x5f3759df - (i >> 1);
    y = unsafe { mem::transmute(i) };

    y = y * (1.5 - (x2 * y * y));
    y

}

pub fn matrix_to_quaternion(m: &Matrix4<f32>) -> Quaternion<f32> {

    let mut q = [0.0, 0.0, 0.0, 0.0];

    let next = [1, 2, 0];

    let trace = m[0][0] + m[1][1] + m[2][2];

    if trace > 0.0 {

        let t = trace + 1.0;
        let s = inv_sqrt(t) * 0.5;

        q[3] = s * t;
        q[0] = (m[1][2] - m[2][1]) * s;
        q[1] = (m[2][0] - m[0][2]) * s;
        q[2] = (m[0][1] - m[1][0]) * s;

    } else {

        let mut i = 0;

        if m[1][1] > m[0][0] {
            i = 1;
        }

        if m[2][2] > m[i][i] {
            i = 2;
        }

        let j = next[i];
        let k = next[j];

        let t = (m[i][i] - (m[j][j] + m[k][k])) + 1.0;
        let s = inv_sqrt(t) * 0.5;

        q[i] = s * t;
        q[3] = (m[j][k] - m[k][j]) * s;
        q[j] = (m[i][j] + m[j][i]) * s;
        q[k] = (m[i][k] + m[k][i]) * s;

    }

    (q[3], [q[0], q[1], q[2]])
}

///
/// See http://www.euclideanspace.com/maths/geometry/rotations/conversions/matrixToQuaternion/
///
pub fn quaternion_to_matrix(q: Quaternion<f32>) -> Matrix4<f32> {

    let w = q.0;
    let x = q.1[0];
    let y = q.1[1];
    let z = q.1[2];

    let x2 = x + x;
    let y2 = y + y;
    let z2 = z + z;

    let xx2 = x2 * x;
    let xy2 = x2 * y;
    let xz2 = x2 * z;

    let yy2 = y2 * y;
    let yz2 = y2 * z;
    let zz2 = z2 * z;

    let wy2 = y2 * w;
    let wz2 = z2 * w;
    let wx2 = x2 * w;

    [
        [1.0 - yy2 - zz2, xy2 + wz2, xz2 - wy2, 0.0],
        [xy2 - wz2, 1.0 - xx2 - zz2, yz2 + wx2, 0.0],
        [xz2 + wy2, yz2 - wx2, 1.0 - xx2 - yy2, 0.0],
        [0.0, 0.0,  0.0,  1.0]
    ]

}
