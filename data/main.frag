#version 460

layout(location = 0) in vec3 vPosition;
layout(location = 1) in vec3 vNormal;
layout(location = 2) in vec2 vUV;
layout(location = 3) in vec3 vCamera;

layout(location = 0) out vec4 color;
layout(location = 1) out vec4 normal;
layout(location = 2) out vec4 specular;

layout(set = 1, binding = 0) uniform texture2DArray modelTexture;
layout(set = 1, binding = 1) uniform sampler modelSampler;

void main() {
	vec3 n = normalize(vNormal); // renormalize to hide problems w/ shader parameter interpolation
	vec3 l = normalize(vec3(100.0, 100.0, 100.0)); // light direction
	vec3 v = normalize(vCamera - vPosition); // view direction
	vec3 h = normalize(l + v); // halfway direction

	// calculate brdf
	float brdf = 0.0;
	{
		float roughness = 0.7;

		float alpha = pow(roughness, 2.0);
		float d = pow(alpha, 2.0) / (3.14 * pow((pow(dot(n, h), 2.0) * (alpha * alpha - 1.0) + 1.0), 2.0));

		float k = pow(roughness + 1, 2) / 8.0;
		float g_l = max(dot(n, l), 0.0) / (dot(n, l) * (1 - k) + k);
		float g_v = max(dot(n, v), 0.0) / (dot(n, v) * (1 - k) + k);
		float g = g_l * g_v;

		float bias = dot(n, l);
		float f = bias + (1 - bias) * pow(2.0, (-5.55473 * dot(v, h) - 6.98316) * dot(v, h));

		brdf = (d * f * g) / 4.0 * dot(n, l) * dot(n, v);
	}

	vec4 diffuse = texture(sampler2DArray(modelTexture, modelSampler), vec3(vUV, 0)) * max(dot(l, n), 0.01);
	vec4 ambient = vec4(0.2, 0.2, 0.2, 1.0) * 0.02;

	// upload to the G-buffer
	color = ambient + diffuse;
	normal = vec4((vNormal + vec3(1.0, 1.0, 1.0)) * 0.5, 1.0);
	specular = vec4(0.5, 0.5, 0.5, 1.0) * brdf;
}
