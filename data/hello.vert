#version 450 core

layout(location = 0) in vec2 vVertex;
layout(location = 1) in vec4 vColor;

layout(location = 0) out vec4 color;

void main() {
	gl_Position = vec4(vVertex, 0.0, 1.0);
	color = vColor;
}
