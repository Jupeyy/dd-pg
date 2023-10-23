#version 450
#extension GL_ARB_separate_shader_objects : enable

#ifdef TW_TEXTURED
layout(binding = 0) uniform sampler2D gTextureSampler;
#endif

layout(location = 0) noperspective in vec2 texCoord;
layout(location = 1) noperspective in vec4 vertColor;

layout(location = 0) out vec4 FragClr;
void main()
{
#ifdef TW_TEXTURED
	vec4 tex = texture(gTextureSampler, texCoord);
	FragClr = tex * vertColor;
#else
	FragClr = vertColor;
#endif
#ifdef TW_NO_ALPHA
	FragClr.a = 1.0;
#endif
}
