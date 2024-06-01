struct In {
	@location(0) pos: vec2<f32>,
	@location(1) tex: vec2<f32>,
	@location(2) color: vec4<f32>,
}

struct PosBO {
	pos: mat4x2<f32>,
}
var<push_constant> g_pos: PosBO;

struct Out {
    @builtin(position) position: vec4<f32>,
	@location(0) @interpolate(linear) tex: vec2<f32>,
	@location(1) @interpolate(linear) color: vec4<f32>,
}

@vertex
fn main(in: In) -> Out {
	var out = Out();
	out.position = vec4<f32>(g_pos.pos * vec4<f32>(in.pos, 0.0, 1.0), 0.0, 1.0);
	out.tex = in.tex;
	out.color = in.color;
	return out;
}
