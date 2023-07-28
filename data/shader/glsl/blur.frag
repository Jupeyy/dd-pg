#version 450
#extension GL_ARB_separate_shader_objects : enable

// credit: https://stackoverflow.com/a/64845819
layout (binding = 0) uniform sampler2D gBlurTexture;

layout(push_constant) uniform SBlueProps {
	layout(offset = 0) vec2 gTextureSize;
    layout(offset = 8) float gBlurRadius;
} gBlur;

layout (location = 1) noperspective in vec4 vertColor;
layout (location = 2) noperspective in vec2 Pos;
layout (location = 0) out vec4 FragClr;

void main()
{
    float x;
    float y;
    float xx;
    float yy;
    float rr = gBlur.gBlurRadius * gBlur.gBlurRadius;
    float dx;
    float dy;
    float w;
    float w0;
    w0 = 0.3780 / pow(gBlur.gBlurRadius, 1.975);
    vec2 p;
    vec4 col = vec4(0.0, 0.0, 0.0, 0.0);
    for (dx = 1.0 / gBlur.gTextureSize.x, x = -gBlur.gBlurRadius, p.x = 0.5 + (Pos.x * 0.5) + (x * dx); x <= gBlur.gBlurRadius; x++, p.x += dx) {
        xx = x * x;
        for (dy = 1.0 / gBlur.gTextureSize.y, y = -gBlur.gBlurRadius, p.y = 0.5 + (Pos.y * 0.5) + (y * dy); y <= gBlur.gBlurRadius; y++, p.y += dy) {
            yy = y * y;
            if (xx + yy <= rr)
            {
                w = w0 * exp((-xx - yy) / (2.0 * rr));
                col += texture(gBlurTexture, p) * w;
            }
        }
    }
    FragClr = col;
}
