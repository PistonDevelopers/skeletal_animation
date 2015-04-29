#version 150 core

// Dual-Quaternion Linear Blend Skinning
// Reference: http://www.seas.upenn.edu/~ladislav/kavan07skinning/kavan07skinning.pdf

uniform mat4 u_model_view_proj;
uniform mat4 u_model_view;

const int MAX_JOINTS = 64;

uniform u_skinning_transforms {
    mat2x4 skinning_transforms[MAX_JOINTS];
};

in vec3 pos, normal;
in vec2 uv;

in ivec4 joint_indices;
in vec4 joint_weights;

out vec3 v_normal;
out vec2 v_TexCoord;

mat4 dualQuaternionToMatrix(vec4 qReal, vec4 qDual) {

	mat4 M;

	float len2 = dot(qReal, qReal);
	float w = qReal.x, x = qReal.y, y = qReal.z, z = qReal.w;
	float t0 = qDual.x, t1 = qDual.y, t2 = qDual.z, t3 = qDual.w;

	M[0][0] = w*w + x*x - y*y - z*z; M[0][1] = 2*x*y - 2*w*z; M[0][2] = 2*x*z + 2*w*y;
	M[1][0] = 2*x*y + 2*w*z; M[1][1] = w*w + y*y - x*x - z*z; M[1][2] = 2*y*z - 2*w*x;
	M[2][0] = 2*x*z - 2*w*y; M[2][1] = 2*y*z + 2*w*x; M[2][2] = w*w + z*z - x*x - y*y;

	M[0][3] = -2*t0*x + 2*w*t1 - 2*t2*z + 2*y*t3;
	M[1][3] = -2*t0*y + 2*t1*z - 2*x*t3 + 2*w*t2;
	M[2][3] = -2*t0*z + 2*x*t2 + 2*w*t3 - 2*t1*y;

	M /= len2;

	return M;
}

void main() {
    v_TexCoord = vec2(uv.x, 1 - uv.y);

    float wx = joint_weights.x;
    float wy = joint_weights.y;
    float wz = joint_weights.z;
    float wa = joint_weights.a;

    if (dot(skinning_transforms[joint_indices.x][0],
            skinning_transforms[joint_indices.y][0]) < 0.0) { wy *= -1; }

    if (dot(skinning_transforms[joint_indices.x][0],
            skinning_transforms[joint_indices.z][0]) < 0.0) { wz *= -1; }

    if (dot(skinning_transforms[joint_indices.x][0],
            skinning_transforms[joint_indices.a][0]) < 0.0) { wa *= -1; }

    mat2x4 blendedSkinningDQ = skinning_transforms[joint_indices.x] * wx;
    blendedSkinningDQ += skinning_transforms[joint_indices.y] * wy;
    blendedSkinningDQ += skinning_transforms[joint_indices.z] * wz;
    blendedSkinningDQ += skinning_transforms[joint_indices.a] * wa;
    blendedSkinningDQ /= length(blendedSkinningDQ[0]);

    mat4 blendedSkinningMatrix = dualQuaternionToMatrix(blendedSkinningDQ[0], blendedSkinningDQ[1]);
    vec4 bindPoseVertex = vec4(pos, 1.0);
    vec4 bindPoseNormal = vec4(normal, 0.0);

    vec4 adjustedVertex = bindPoseVertex * blendedSkinningMatrix;
    vec4 adjustedNormal = bindPoseNormal * blendedSkinningMatrix;

    gl_Position = u_model_view_proj * adjustedVertex;
    v_normal = normalize(u_model_view * adjustedNormal).xyz;
}
