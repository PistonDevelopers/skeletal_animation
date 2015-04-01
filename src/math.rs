use vecmath::Matrix4;
use quaternion::Quaternion;

///
/// Works, but not ideal. Seeing some popping in animation due to errors...
/// See http://www.euclideanspace.com/maths/geometry/rotations/conversions/matrixToQuaternion/
///
pub fn matrix_to_quaternion(m: &Matrix4<f32>) -> Quaternion<f32> {
    let trace = m[0][0] + m[1][1] + m[2][2];
    if (trace + 1.0) > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        let w = s * 0.25;
        let x = (m[1][2] - m[2][1]) / s;
        let y = (m[2][0] - m[0][2]) / s;
        let z = (m[0][1] - m[1][0]) / s;
        (w, [x, y, z])
    } else if ((m[0][0] > m [1][1]) && (m[0][0] > m[2][2])) {

        let s = (1.0 + m[0][0] - m[1][1] - m[2][2]).sqrt() * 2.0;
        let w = (m[1][2] - m[2][1]) / s;
        let x = s * 0.25;
        let y = (m[1][0] - m[0][1]) / s;
        let z = (m[2][0] - m[0][2]) / s;
        (w, [x, y, z])

    } else if (m[1][1] > m [2][2]) {

        let s = (1.0 + m[1][1] - m[0][0] - m[2][2]).sqrt() * 2.0;
        let w = (m[2][0] - m[0][2]) / s;
        let x = (m[1][0] - m[0][1]) / s;
        let y = s * 0.25;
        let z = (m[2][1] - m[1][2]) / s;
        (w, [x, y, z])

    } else {

        let s = (1.0 + m[2][2] - m[0][0] - m[1][1]).sqrt() * 2.0;
        let w = (m[0][1] - m[1][0]) / s;
        let x = (m[2][0] - m[0][2]) / s;
        let y = (m[2][1] - m[1][2]) / s;
        let z = s * 0.25;
        (w, [x, y, z])
    }
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
