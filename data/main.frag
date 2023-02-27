#version 460

layout(location = 0) in vec3 vPosition;
layout(location = 1) in vec3 vNormal;
layout(location = 2) in vec2 vUV;
layout(location = 3) in vec3 vCamera;
layout(location = 4) in float roughness;

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

	float n_dot_l = dot(n, l);
	float n_dot_v = dot(n, v);
	float l_dot_h = dot(l, h);

	// calculate brdf
	float brdf = 0.0;
	{
		float alpha = pow(roughness, 2.0);
		float d = pow(alpha, 2.0) / (3.14 * pow((pow(dot(n, h), 2.0) * (pow(alpha, 2.0) - 1.0) + 1.0), 2.0));

		float k = pow(roughness + 1, 2) / 8.0;
		float g_l = max(n_dot_l, 0.0) / (n_dot_l * (1 - k) + k);
		float g_v = max(n_dot_v, 0.0) / (n_dot_v * (1 - k) + k);
		float g = g_l * g_v;

		float bias = 0.04;
		float f = bias + (1 - bias) * pow(2.0, (-5.55473 * l_dot_h - 6.98316) * l_dot_h);

		brdf = (d * f * g) / (4.0 * n_dot_l * n_dot_v);
	}

	vec4 diffuse = texture(sampler2DArray(modelTexture, modelSampler), vec3(vUV, 0)) * max(n_dot_l, 0.01);
	vec4 ambient = vec4(0.2, 0.2, 0.2, 1.0) * 0.02;

	// upload to the G-buffer
	color = ambient + diffuse;
	normal = vec4((vNormal + vec3(1.0, 1.0, 1.0)) * 0.5, 1.0);
	specular = vec4(0.5, 0.5, 0.5, 1.0) * brdf;
}
