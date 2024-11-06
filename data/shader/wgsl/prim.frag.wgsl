#ifdef TW_TEXTURED
@group(0) @binding(0) var g_texture: texture_2d<f32>;
@group(1) @binding(0) var g_sampler: sampler;
#endif

struct In {
	@location(0) @interpolate(linear) tex: vec2<f32>,
	@location(1) @interpolate(linear) color: vec4<f32>,
}

struct Out {
	@location(0) color: vec4<f32>,
}

@fragment
fn main(in: In) -> Out
{
	var out = Out();
#ifdef TW_TEXTURED
	var tex = textureSample(g_texture, g_sampler, in.tex);
	out.color = tex * in.color;
#else
	out.color = in.color;
#endif
	return out;
}