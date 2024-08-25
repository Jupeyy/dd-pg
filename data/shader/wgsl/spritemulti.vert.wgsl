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
@group(2) @binding(0)
var<uniform> g_rsp: array<vec4<f32>, 512>;

@vertex
fn main(in: In) -> Out {
	var out = Out();
	var final_pos = in.pos;
	if(g_rsp[in.instance_index].w != 0.0) {
		var x = final_pos.x - g_proj.center.x;
		var y = final_pos.y - g_proj.center.y;
		
		final_pos.x = x * cos(g_rsp[in.instance_index].w) - y * sin(g_rsp[in.instance_index].w) + g_proj.center.x;
		final_pos.y = x * sin(g_rsp[in.instance_index].w) + y * cos(g_rsp[in.instance_index].w) + g_proj.center.y;
	}
	
	final_pos.x *= g_rsp[in.instance_index].z;
	final_pos.y *= g_rsp[in.instance_index].z;

	final_pos.x += g_rsp[in.instance_index].x;
	final_pos.y += g_rsp[in.instance_index].y;

	out.position = vec4(g_proj.pos * vec4(final_pos, 0.0, 1.0), 0.0, 1.0);
	out.tex = in.tex;
	out.color = in.color;
	return out;
}
