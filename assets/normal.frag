#version 330 core
in vec3 v_pos;
in vec2 v_uv;
in vec3 v_normal;

out vec4 f_color;

void main() {
    f_color = vec4(v_normal, 1.0);
}