#version 460

layout(location = 0) in vec3 vVertex;
layout(location = 1) in vec4 vBoneWeights;
layout(location = 2) in uvec4 vBoneIndices;

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
}
