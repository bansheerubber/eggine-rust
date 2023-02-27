#version 460

layout(location = 0) in vec3 vVertex;
layout(location = 1) in vec3 vNormal;
layout(location = 2) in vec2 vUV;

layout(location = 0) out vec3 position;
layout(location = 1) out vec3 normal;
layout(location = 2) out vec2 uv;
layout(location = 3) out vec3 camera;
layout(location = 4) out float roughness;

layout(std140, set = 0, binding = 0) uniform vertexBlock
{
	vec3 cameraPosition;
	mat4 perspective;
	mat4 view;
} vb;

struct ObjectData {
	mat4 model;
	vec4 textureOffset;
	float roughness;
};

layout(std140, set = 0, binding = 1) readonly buffer objectBlock
{
	ObjectData objects[];
} ob;

void main() {
	gl_Position = vb.perspective * vb.view * ob.objects[gl_DrawID].model * vec4(vVertex, 1.0);

	// pass some stuff to the fragment shader
	position = vVertex;
	normal = vNormal;
	uv = vUV * ob.objects[gl_DrawID].textureOffset.z + ob.objects[gl_DrawID].textureOffset.xy;
	camera = vb.cameraPosition;

	// material properties
	roughness = ob.objects[gl_DrawID].roughness;
}
