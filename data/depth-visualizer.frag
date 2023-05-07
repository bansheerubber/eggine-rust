#version 460

layout(location = 0) in vec2 inUV;

layout(location = 0) out vec4 color;

layout(set = 0, binding = 0) uniform texture2D depthTexture;
layout(set = 0, binding = 1) uniform sampler depthSampler;

layout(push_constant) uniform block
{
	vec2 clipping;
};

void main() {
	float depth = texture(sampler2D(depthTexture, depthSampler), vec2(inUV.x, 1.0 - inUV.y)).r;
	float near = clipping.x;
	float far = clipping.y;

	// http://www.geeks3d.com/20091216/geexlab-how-to-visualize-the-depth-buffer-in-glsl/
	float linearized = (2.0 * near) / (far + near - depth * (far - near));
	color = vec4(vec3(linearized), 1.0);
}
