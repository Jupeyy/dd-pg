struct In {
	@builtin(instance_index) instance_index: u32,
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
	center: vec2<f32>,
}
var<push_constant> g_proj: ProjectionMat;

/// Rotation, scaling, positioning
struct Rsp {
	pos: vec2<f32>,
	scale: f32,
	rotation: f32,
	color: vec4<f32>,
}
@group(2) @binding(0)
var<uniform> g_rsp: array<Rsp, 256>;

@vertex
fn main(in: In) -> Out {
	var out = Out();
	var final_pos = in.pos;
	if(g_rsp[in.instance_index].rotation != 0.0) {
		var x = final_pos.x - g_proj.center.x;
		var y = final_pos.y - g_proj.center.y;
		
		final_pos.x = x * cos(g_rsp[in.instance_index].rotation) - y * sin(g_rsp[in.instance_index].rotation) + g_proj.center.x;
		final_pos.y = x * sin(g_rsp[in.instance_index].rotation) + y * cos(g_rsp[in.instance_index].rotation) + g_proj.center.y;
	}
	
	final_pos.x *= g_rsp[in.instance_index].scale;
	final_pos.y *= g_rsp[in.instance_index].scale;

	final_pos.x += g_rsp[in.instance_index].pos.x;
	final_pos.y += g_rsp[in.instance_index].pos.y;

	out.position = vec4(g_proj.pos * vec4(final_pos, 0.0, 1.0), 0.0, 1.0);
	out.tex = in.tex;
	out.color = in.color * g_rsp[in.instance_index].color;
	return out;
}
