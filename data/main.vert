#version 460

layout(location = 0) in vec3 vVertex;
layout(location = 1) in vec3 vNormal;
layout(location = 2) in vec2 vUV;
layout(location = 3) in vec4 vBoneWeights;
layout(location = 4) in uvec4 vBoneIndices;

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
	uint boneOffset;
};

layout(std140, set = 0, binding = 1) readonly buffer objectBlock
{
	ObjectData objects[];
} ob;

// how to index: gl_DrawID + boneIndex
layout(std140, set = 0, binding = 2) readonly buffer boneMatrices
{
	mat4 matrices[];
} bones;

void main() {
	uint boneOffset = ob.objects[gl_DrawID].boneOffset;

	mat4 skinMatrix = mat4(1.0);
	if (vBoneWeights.x + vBoneWeights.y + vBoneWeights.z + vBoneWeights.w > 0.9) {
		skinMatrix =
			vBoneWeights.x * bones.matrices[boneOffset + vBoneIndices.x]
			+ vBoneWeights.y * bones.matrices[boneOffset + vBoneIndices.y]
			+ vBoneWeights.z * bones.matrices[boneOffset + vBoneIndices.z]
			+ vBoneWeights.w * bones.matrices[boneOffset + vBoneIndices.w];
	}

	gl_Position = vb.perspective * vb.view * ob.objects[gl_DrawID].model * skinMatrix * vec4(vVertex, 1.0);

	// pass some stuff to the fragment shader
	position = vVertex;
	normal = mat3(skinMatrix) * vNormal;
	uv = vUV * ob.objects[gl_DrawID].textureOffset.z + ob.objects[gl_DrawID].textureOffset.xy;
	camera = vb.cameraPosition;

	// material properties
	roughness = ob.objects[gl_DrawID].roughness;
}
