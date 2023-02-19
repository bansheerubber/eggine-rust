#version 450 core

layout(location = 0) in vec3 vVertex;
layout(location = 1) in vec3 vColor;
layout(location = 2) in vec3 vNormal;

layout(location = 0) out vec3 color;
layout(location = 1) out vec3 normal;
layout(location = 2) out vec3 position;
layout(location = 3) out vec3 camera;

layout(std140, set = 0, binding = 0) uniform vertexBlock
{
	vec3 cameraPosition;
	mat4 perspective;
	mat4 view;
} vb;

void main() {
	gl_Position = vb.perspective * vb.view * vec4(vVertex, 1.0);

	// pass some stuff to the fragment shader
	color = vColor;
	normal = vNormal;
	position = vVertex;
	camera = vb.cameraPosition;
}
