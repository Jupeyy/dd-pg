struct In {
	@location(0) pos: vec2<f32>,
	@location(1) color: vec4<f32>,
	@location(2) tex: vec3<f32>,
}

struct Out {
    @builtin(position) position: vec4<f32>,
	@location(0) color: vec4<f32>,
#ifdef TW_TEXTURED
	@location(1) tex: vec3<f32>,
#endif
}

struct ProjectionMat {
	pos: mat4x2<f32>,
}
var<push_constant> g_proj: ProjectionMat;

@vertex
fn main(in: In) -> Out
{
	var out = Out();
	out.position = vec4(g_proj.pos * vec4(in.pos, 0.0, 1.0), 0.0, 1.0);
#ifdef TW_TEXTURED
	out.tex = in.tex;
#endif
	out.color = in.color;
	return out;
}
