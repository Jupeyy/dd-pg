struct In {
	@location(0) @interpolate(linear) tex: vec2<f32>,
	@location(1) @interpolate(linear) color: vec4<f32>,
}

struct Out {
	@location(0) color: vec4<f32>,
}

@group(0) @binding(0) var g_texture: texture_2d<f32>;
@group(1) @binding(0) var g_sampler: sampler;

struct FragColor {
	// 48 padding
	padding: array<f32, 12>,
	color: vec4<f32>,
}
var<push_constant> g_color: FragColor;

@fragment
fn main(in: In) -> Out {
	var out = Out();
	var tex = textureSample(g_texture, g_sampler, in.tex);
	out.color = tex * in.color * g_color.color;
	return out;
}
