struct In {
	@location(0) pos: vec2<f32>,
	@location(1) tex: vec2<f32>,
	@location(2) color: vec4<f32>,
}

struct Out {
    @builtin(position) position: vec4<f32>,
	@location(0) @interpolate(linear) tex: vec2<f32>,
	@location(1) @interpolate(linear) color: vec4<f32>,
}

struct ProjectionMat {
	pos: mat4x2<f32>,
#ifndef TW_ROTATIONLESS
	center: vec2<f32>,
	rotation: f32,
#endif
}
var<push_constant> g_proj: ProjectionMat;

@vertex
fn main(in: In) -> Out
{
	var out = Out();
	var final_pos = in.pos;
#ifndef TW_ROTATIONLESS
	var x = final_pos.x - g_proj.center.x;
	var y = final_pos.y - g_proj.center.y;
	
	final_pos.x = x * cos(g_proj.rotation) - y * sin(g_proj.rotation) + g_proj.center.x;
	final_pos.y = x * sin(g_proj.rotation) + y * cos(g_proj.rotation) + g_proj.center.y;
#endif

	out.position = vec4(g_proj.pos * vec4(final_pos, 0.0, 1.0), 0.0, 1.0);
	out.tex = in.tex;
	out.color = in.color;
	return out;
}
