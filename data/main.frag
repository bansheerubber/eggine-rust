#version 460

layout(location = 0) in vec3 vColor;
layout(location = 1) in vec3 vNormal;
layout(location = 2) in vec3 vPosition;
layout(location = 3) in vec3 vCamera;

layout(location = 0) out vec4 color;
layout(location = 1) out vec4 normal;
layout(location = 2) out vec4 specular;

void main() {
	vec3 normalizedNormal = normalize(vNormal); // renormalize to hide problems w/ shader parameter interpolation

	// calculate specular coefficient
	vec3 lightDirection = normalize(vec3(100.0, 100.0, 100.0));
	vec3 viewDirection = normalize(vCamera - vPosition);
	vec3 halfwayDirection = normalize(lightDirection + viewDirection);
	float specularCoefficient = pow(max(dot(normalizedNormal, halfwayDirection), 0.0), 32.0);

	// calculate diffuse coefficient
	float diffuseCoefficient = max(dot(lightDirection, normalizedNormal), 0.01);

	// calculate ambient coefficient
	float ambientCoefficient = 0.02;

	// upload to the G-buffer
	color = vec4(1.0, 0.0, 0.0, 1.0) * diffuseCoefficient + vec4(0.2, 0.2, 0.2, 1.0) * ambientCoefficient;
	normal = vec4((vNormal + vec3(1.0, 1.0, 1.0)) * 0.5, 1.0);
	specular = vec4(0.5, 0.5, 0.5, 1.0) * specularCoefficient * diffuseCoefficient;
}
