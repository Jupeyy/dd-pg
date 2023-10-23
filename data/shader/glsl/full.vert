#version 450
#extension GL_ARB_separate_shader_objects : enable

layout (location = 0) noperspective out vec2 texCoord;
layout (location = 1) noperspective out vec4 vertColor;
layout (location = 2) noperspective out vec2 pos;

void main()
{
    vec2 outPos = vec2((gl_VertexIndex << 1) & 2, gl_VertexIndex & 2);

    pos = vec2(outPos * 2.0f + -1.0f);
    gl_Position = vec4(pos, 0.0, 1.0);
    texCoord = outPos;
    vertColor = vec4(1, 1, 1, 1);
}
