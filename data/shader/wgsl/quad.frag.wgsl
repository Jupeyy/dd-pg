struct In {
	@location(0) @interpolate(linear) color: vec4<f32>,
	@location(1) @interpolate(flat) index: u32,
#ifdef TW_TEXTURED
	@location(2) @interpolate(linear) tex: vec2<f32>,
#endif
}

struct Out {
	@location(0) color: vec4<f32>,
}

#ifdef TW_TEXTURED
@group(0) @binding(0) var g_texture: texture_2d<f32>;
@group(1) @binding(0) var g_sampler: sampler;
#endif

struct Quad {
	color: vec4<f32>,
	offset: vec2<f32>,
	rotation: f32,
	// padding 4 bytes
}

const TW_MAX_QUADS = 256;

#ifdef TW_TEXTURED
const QUADS_SET_INDEX = 2;
#else
const QUADS_SET_INDEX = 0;
#endif
@group(QUADS_SET_INDEX)
@binding(0)
var<uniform> g_quads: array<Quad, TW_MAX_QUADS>;

struct ProjectionMat {
	pos: mat4x2<f32>,
	quad_offset: u32,
}
var<push_constant> g_proj: ProjectionMat;

@fragment
fn main(in: In) -> Out {
	var out = Out();
#ifdef TW_TEXTURED
	var tex_color = textureSample(g_texture, g_sampler, in.tex);
	out.color = tex_color * in.color * g_quads[in.index].color;
#else
	out.color = in.color * g_quads[in.index].color;
#endif
	return out;
}
