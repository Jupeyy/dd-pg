struct In {
	@location(0) pos: vec2<f32>,
#ifdef TW_TEXTURED
	@location(1) tex3d_and_flags: vec4<u32>,
#endif
}

struct Out {
    @builtin(position) position: vec4<f32>,
#ifdef TW_TEXTURED
	@location(0) @interpolate(linear, centroid) tex: vec3<f32>,
#endif
}

struct PosBO {
	pos: mat4x2<f32>,
	offset: vec2<f32>,
	scale: vec2<f32>,
}
var<push_constant> g_pos: PosBO;

@vertex
fn main(in: In) -> Out {
	var out = Out();
	// scale then position vertex
	var pos = (in.pos * g_pos.scale) + g_pos.offset;
	out.position = vec4(g_pos.pos * vec4(pos, 0.0, 1.0), 0.0, 1.0);

#ifdef TW_TEXTURED
	// scale the texture coordinates too
	var tex_scale = g_pos.scale;
	if in.tex3d_and_flags.w > 0 {
		tex_scale = g_pos.scale.yx;
	}
	out.tex = vec3(vec2<f32>(in.tex3d_and_flags.xy) * tex_scale, f32(in.tex3d_and_flags.z));
#endif
	return out;
}
