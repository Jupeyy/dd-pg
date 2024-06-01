@group(0) @binding(0) var g_texture: texture_2d<f32>;
@group(1) @binding(0) var g_sampler: sampler;

struct BlurProps {
	// 32 padding
	padding: array<f32, 8>,
	tex_size: vec2<f32>,
	scale: vec2<f32>,
	color: vec4<f32>,
	radius: f32,
}
var<push_constant> g_blur: BlurProps;

struct In {
	@location(0) @interpolate(linear) pos: vec2<f32>,
	@location(1) @interpolate(linear) color: vec4<f32>,
}

struct Out {
	@location(0) color: vec4<f32>,
}

const PI = 3.1415926538;

// MIT LICENSE: https://github.com/Jam3/glsl-fast-gaussian-blur (commit: 5dbb6e97aa43d4be9369bdd88e835f47023c5e2a)
// slightly simplified
fn blur5(uv: vec2<f32>, resolution: vec2<f32>, direction: vec2<f32>) -> vec4<f32> {
	var color = vec4(0.0);
	var off1 = vec2(1.3333333333333333) * direction;
	color += textureSample(g_texture, g_sampler, uv) * 0.29411764705882354;
	color += textureSample(g_texture, g_sampler, uv + (off1 / resolution)) * 0.35294117647058826;
	color += textureSample(g_texture, g_sampler, uv - (off1 / resolution)) * 0.35294117647058826;
	return color; 
}

fn blur9(uv: vec2<f32>, resolution: vec2<f32>, direction: vec2<f32>) -> vec4<f32>  {
	var color = vec4(0.0);
	var off1 = vec2(1.3846153846) * direction;
	var off2 = vec2(3.2307692308) * direction;
	color += textureSample(g_texture, g_sampler, uv) * 0.2270270270;
	color += textureSample(g_texture, g_sampler, uv + (off1 / resolution)) * 0.3162162162;
	color += textureSample(g_texture, g_sampler, uv - (off1 / resolution)) * 0.3162162162;
	color += textureSample(g_texture, g_sampler, uv + (off2 / resolution)) * 0.0702702703;
	color += textureSample(g_texture, g_sampler, uv - (off2 / resolution)) * 0.0702702703;
	return color;
}

fn blur13(uv: vec2<f32>, resolution: vec2<f32>, direction: vec2<f32>) -> vec4<f32>  {
	var color = vec4(0.0);
	var off1 = vec2(1.411764705882353) * direction;
	var off2 = vec2(3.2941176470588234) * direction;
	var off3 = vec2(5.176470588235294) * direction;
	color += textureSample(g_texture, g_sampler, uv) * 0.1964825501511404;
	color += textureSample(g_texture, g_sampler, uv + (off1 / resolution)) * 0.2969069646728344;
	color += textureSample(g_texture, g_sampler, uv - (off1 / resolution)) * 0.2969069646728344;
	color += textureSample(g_texture, g_sampler, uv + (off2 / resolution)) * 0.09447039785044732;
	color += textureSample(g_texture, g_sampler, uv - (off2 / resolution)) * 0.09447039785044732;
	color += textureSample(g_texture, g_sampler, uv + (off3 / resolution)) * 0.010381362401148057;
	color += textureSample(g_texture, g_sampler, uv - (off3 / resolution)) * 0.010381362401148057;
	return color;
}

@fragment
fn main(in: In) -> Out
{
	var out = Out();
	var dir = g_blur.scale;

	var blurred = vec4(0.0, 0.0, 0.0, 0.0);
	if g_blur.radius <= 5.0 {
			blurred += blur5(in.pos, g_blur.tex_size, dir);
	}
	else if g_blur.radius <= 9.0 {
			blurred += blur9(in.pos, g_blur.tex_size, dir);
	}
	else if g_blur.radius <= 13.0 {
			blurred += blur13(in.pos, g_blur.tex_size, dir);
	}

	var original_pixel = textureSample(g_texture, g_sampler, in.pos);
	out.color = vec4(
		vec3(blurred.rgb * (1.0 - g_blur.color.w) + g_blur.color.rgb * g_blur.color.w) * 
			original_pixel.w + original_pixel.rgb * (1.0 - original_pixel.w),
		original_pixel.w
	);
	return out;
}
