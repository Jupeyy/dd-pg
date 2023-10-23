#version 450
#extension GL_ARB_separate_shader_objects : enable

layout (binding = 0) uniform sampler2D gBlurTexture;

layout(push_constant) uniform SBlueProps {
	layout(offset = 0) vec2 gTextureSize;
		layout(offset = 8) float gBlurRadius;
		layout(offset = 12) int gIsHorizontal;
		layout(offset = 16) vec4 gColor;
} gBlur;

layout (location = 1) noperspective in vec4 vertColor;
layout (location = 2) noperspective in vec2 Pos;
layout (location = 0) out vec4 FragClr;

#define PI 3.1415926538

// MIT LICENSE: https://github.com/Jam3/glsl-fast-gaussian-blur (commit: 5dbb6e97aa43d4be9369bdd88e835f47023c5e2a)
// slightly simplified
vec4 blur5(vec2 uv, vec2 resolution, vec2 direction) {
	vec4 color = vec4(0.0);
	vec2 off1 = vec2(1.3333333333333333) * direction;
	color += texture(gBlurTexture, uv) * 0.29411764705882354;
	color += texture(gBlurTexture, uv + (off1 / resolution)) * 0.35294117647058826;
	color += texture(gBlurTexture, uv - (off1 / resolution)) * 0.35294117647058826;
	return color; 
}

vec4 blur9(vec2 uv, vec2 resolution, vec2 direction) {
	vec4 color = vec4(0.0);
	vec2 off1 = vec2(1.3846153846) * direction;
	vec2 off2 = vec2(3.2307692308) * direction;
	color += texture(gBlurTexture, uv) * 0.2270270270;
	color += texture(gBlurTexture, uv + (off1 / resolution)) * 0.3162162162;
	color += texture(gBlurTexture, uv - (off1 / resolution)) * 0.3162162162;
	color += texture(gBlurTexture, uv + (off2 / resolution)) * 0.0702702703;
	color += texture(gBlurTexture, uv - (off2 / resolution)) * 0.0702702703;
	return color;
}

vec4 blur13(vec2 uv, vec2 resolution, vec2 direction) {
	vec4 color = vec4(0.0);
	vec2 off1 = vec2(1.411764705882353) * direction;
	vec2 off2 = vec2(3.2941176470588234) * direction;
	vec2 off3 = vec2(5.176470588235294) * direction;
	color += texture(gBlurTexture, uv) * 0.1964825501511404;
	color += texture(gBlurTexture, uv + (off1 / resolution)) * 0.2969069646728344;
	color += texture(gBlurTexture, uv - (off1 / resolution)) * 0.2969069646728344;
	color += texture(gBlurTexture, uv + (off2 / resolution)) * 0.09447039785044732;
	color += texture(gBlurTexture, uv - (off2 / resolution)) * 0.09447039785044732;
	color += texture(gBlurTexture, uv + (off3 / resolution)) * 0.010381362401148057;
	color += texture(gBlurTexture, uv - (off3 / resolution)) * 0.010381362401148057;
	return color;
}

void main()
{
	vec2 Dir = vec2(1.0, 0.0);
	if (gBlur.gIsHorizontal == 0) {
			Dir = vec2(0.0, 1.0);
	}

	vec4 blurred = vec4(0.0, 0.0, 0.0, 0.0);
	if (gBlur.gBlurRadius <= 5.0) {
			blurred += blur5(vec2(0.5, 0.5) + (Pos * 0.5), gBlur.gTextureSize, Dir);
	}
	else if (gBlur.gBlurRadius <= 9.0) {
			blurred += blur9(vec2(0.5, 0.5) + (Pos * 0.5), gBlur.gTextureSize, Dir);
	}
	else if (gBlur.gBlurRadius <= 13.0) {
			blurred += blur13(vec2(0.5, 0.5) + (Pos * 0.5), gBlur.gTextureSize, Dir);
	}
	FragClr = vec4(blurred.rgb, 1.0) * (1.0 - gBlur.gColor.w) + (gBlur.gColor * gBlur.gColor.w);
}
