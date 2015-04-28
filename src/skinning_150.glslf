#version 150

uniform sampler2D u_texture;

in vec3 v_normal;
out vec4 out_color;

in vec2 v_TexCoord;

void main() {
    vec4 texColor = texture(u_texture, v_TexCoord);

    // unidirectional light in direction as camera
    vec3 light = vec3(0.0, 0.0, 1.0);
    light = normalize(light);
    float intensity = max(dot(v_normal, light), 0.0);

    out_color = vec4(intensity, intensity, intensity, 1.0) * texColor;
}