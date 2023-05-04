#version 450 core

layout(location = 0) in vec2 inUV;

layout(location = 0) out vec4 color;

layout(set = 0, binding = 0) uniform texture2D diffuseTexture;
layout(set = 0, binding = 1) uniform sampler diffuseSampler;

layout(set = 0, binding = 2) uniform texture2D normalTexture;
layout(set = 0, binding = 3) uniform sampler normalSampler;

layout(set = 0, binding = 4) uniform texture2D specularTexture;
layout(set = 0, binding = 5) uniform sampler specularSampler;

layout(set = 0, binding = 6) uniform texture2D depthTexture;
layout(set = 0, binding = 7) uniform sampler depthSampler;

void main()
{
	// sample G-buffer
	vec4 diffuseColor = texture(sampler2D(diffuseTexture, diffuseSampler), vec2(inUV.x, 1.0 - inUV.y));
	vec4 specularColor = texture(sampler2D(specularTexture, specularSampler), vec2(inUV.x, 1.0 - inUV.y));
	vec4 normal = texture(sampler2D(normalTexture, normalSampler), vec2(inUV.x, 1.0 - inUV.y));

	// composite results from G-buffer
	color = vec4(vec3(texture(sampler2D(depthTexture, depthSampler), vec2(inUV.x, 1.0 - inUV.y)).r), 1.0);
	// color = diffuseColor + specularColor;
}
