#ifdef TW_TEXTURED
@group(0) @binding(0) var g_texture: texture_2d_array<f32>;
@group(1) @binding(0) var g_sampler: sampler;
#endif

struct In {
	@location(0) @interpolate(linear) color: vec4<f32>,
#ifdef TW_TEXTURED
	@location(1) @interpolate(linear) tex: vec3<f32>,
#endif
}

struct Out {
	@location(0) color: vec4<f32>,
}

@fragment
fn main(in: In) -> Out
{
	var out = Out();
#ifdef TW_TEXTURED
	var tex_color = textureSample(g_texture, g_sampler, in.tex.xy, i32(in.tex.z));
	out.color = tex_color * in.color;
#else
	out.color = in.color;
#endif
	return out;
}

