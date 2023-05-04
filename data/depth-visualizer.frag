#version 460

layout(location = 0) in vec2 inUV;

layout(location = 0) out vec4 color;

layout(set = 0, binding = 0) uniform texture2D depthTexture;
layout(set = 0, binding = 1) uniform sampler depthSampler;

void main() {
	color = vec4(vec3(texture(sampler2D(depthTexture, depthSampler), vec2(inUV.x, 1.0 - inUV.y)).r), 1.0);
}
