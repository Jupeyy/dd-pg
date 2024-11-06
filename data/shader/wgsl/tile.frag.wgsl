#ifdef TW_TEXTURED
struct In {
	@location(0) @interpolate(linear) tex: vec3<f32>,
}
#endif

struct Out {
	@location(0) color: vec4<f32>,
}

#ifdef TW_TEXTURED
@group(0) @binding(0) var g_texture: texture_2d_array<f32>;
@group(1) @binding(0) var g_sampler: sampler;
#endif

struct FragColor {
	padding: array<f32, 16>,
	color: vec4<f32>,
}
var<push_constant> g_color: FragColor;

@fragment
fn main(
#ifdef TW_TEXTURED
	in: In
#endif
) -> Out {
	var out = Out();
#ifdef TW_TEXTURED
	var tex_color = textureSample(g_texture, g_sampler, in.tex.xy, u32(in.tex.z));
	out.color = tex_color * g_color.color;
#else
	out.color = g_color.color;
#endif
	return out;
}
