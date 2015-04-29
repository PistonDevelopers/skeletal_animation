#version 150 core

// Linear Blend Skinning

uniform mat4 u_model_view_proj;
uniform mat4 u_model_view;

const int MAX_JOINTS = 64;

uniform u_skinning_transforms {
    mat4 skinning_transforms[MAX_JOINTS];
};

in vec3 pos, normal;
in vec2 uv;

in ivec4 joint_indices;
in vec4 joint_weights;

out vec3 v_normal;
out vec2 v_TexCoord;

void main() {
    v_TexCoord = vec2(uv.x, 1 - uv.y); // this feels like a bug with gfx?

    vec4 adjustedVertex;
    vec4 adjustedNormal;

    vec4 bindPoseVertex = vec4(pos, 1.0);
    vec4 bindPoseNormal = vec4(normal, 0.0);

    adjustedVertex = bindPoseVertex * skinning_transforms[joint_indices.x] * joint_weights.x;
    adjustedNormal = bindPoseNormal * skinning_transforms[joint_indices.x] * joint_weights.x;

    adjustedVertex = adjustedVertex + bindPoseVertex * skinning_transforms[joint_indices.y] * joint_weights.y;
    adjustedNormal = adjustedNormal + bindPoseNormal * skinning_transforms[joint_indices.y] * joint_weights.y;

    adjustedVertex = adjustedVertex + bindPoseVertex * skinning_transforms[joint_indices.z] * joint_weights.z;
    adjustedNormal = adjustedNormal + bindPoseNormal * skinning_transforms[joint_indices.z] * joint_weights.z;

    // TODO just use remainder for this weight?
    adjustedVertex = adjustedVertex + bindPoseVertex * skinning_transforms[joint_indices.a] * joint_weights.a;
    adjustedNormal = adjustedNormal + bindPoseNormal * skinning_transforms[joint_indices.a] * joint_weights.a;

    gl_Position = u_model_view_proj * adjustedVertex;
    v_normal = normalize(u_model_view * adjustedNormal).xyz;
}
