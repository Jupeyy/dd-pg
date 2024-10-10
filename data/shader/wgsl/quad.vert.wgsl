struct In {
	@builtin(vertex_index) vertex_index: u32,
	@location(0) pos: vec4<f32>,
	@location(1) color: vec4<f32>,
#ifdef TW_TEXTURED
	@location(2) tex: vec2<f32>,
#endif
}

struct Out {
    @builtin(position) position: vec4<f32>,
	@location(0) @interpolate(linear) color: vec4<f32>,
	@location(1) @interpolate(flat) index: u32,
#ifdef TW_TEXTURED
	@location(2) @interpolate(linear) tex: vec2<f32>,
#endif
}

struct Quad {
	color: vec4<f32>,
	offset: vec2<f32>,
	rotation: f32,
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

@vertex
fn main(in: In) -> Out
{
	var out = Out();
	var tmp_quad_index = u32(in.vertex_index / 4u) - g_proj.quad_offset;

	var final_pos = in.pos.xy;
	if g_quads[tmp_quad_index].rotation != 0.0 {
		var x = final_pos.x - in.pos.z;
		var y = final_pos.y - in.pos.w;
		
		final_pos.x = x * cos(g_quads[tmp_quad_index].rotation) - y * sin(g_quads[tmp_quad_index].rotation) + in.pos.z;
		final_pos.y = x * sin(g_quads[tmp_quad_index].rotation) + y * cos(g_quads[tmp_quad_index].rotation) + in.pos.w;
	}

	final_pos.x = final_pos.x + g_quads[tmp_quad_index].offset.x;
	final_pos.y = final_pos.y + g_quads[tmp_quad_index].offset.y;

	out.position = vec4(g_proj.pos * vec4(final_pos, 0.0, 1.0), 0.0, 1.0);
	out.color = in.color;
	out.index = tmp_quad_index;
#ifdef TW_TEXTURED
	out.tex = in.tex;
#endif
	return out;
}
