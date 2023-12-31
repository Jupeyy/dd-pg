use graphics_types::{
    rendering::{BlendType, ColorMaskMode, RenderMode, StencilMode, WrapType},
    types::StreamedQuad,
};
use math::math::vector::{vec2, vec4};

use crate::{graphics::Graphics, streaming::DrawScopeImpl};

pub const DEFAULT_BLUR_RADIUS: f32 = 13.0;
pub const DEFAULT_BLUR_MIX_LENGTH: f32 = 8.0;

fn render_blur_impl(
    graphics: &mut Graphics,
    is_hori: bool,
    blur_radius: f32,
    blur_mix_length: f32,
    blur_color: &vec4,
    is_first: bool,
) {
    let is_last_iter = blur_mix_length <= 1.0;
    let mut quads = graphics.stream_handle.quads_begin();
    quads.set_colors_from_single(1.0, 1.0, 1.0, 1.0);
    quads.quads_set_subset_free(0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0);
    quads.map_canvas(0.0, 0.0, 1.0, 1.0);
    quads.quads_draw_tl(&[StreamedQuad::from_pos_and_size(0.0, 0.0, 1.0, 1.0)]);
    quads.set_stencil_mode(StencilMode::StencilPassed);
    quads.set_color_attachment_texture();
    quads.wrap(WrapType::Clamp);
    quads.set_render_mode(RenderMode::Blur {
        blur_radius,
        scale: if is_hori {
            vec2::new(1.0, 0.0) * blur_mix_length
        } else {
            vec2::new(0.0, 1.0) * blur_mix_length
        },
        blur_color: if !is_hori && is_last_iter {
            *blur_color
        } else {
            vec4::new(1.0, 1.0, 1.0, 0.0)
        },
    });
    quads.blend(BlendType::None);
    if is_first {
        quads.set_color_mask(ColorMaskMode::WriteColorOnly);
    }
    drop(quads);
    let mut quads = graphics.stream_handle.quads_begin();
    quads.set_colors_from_single(1.0, 1.0, 1.0, 1.0);
    quads.quads_set_subset_free(0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0);
    quads.map_canvas(0.0, 0.0, 1.0, 1.0);
    quads.quads_draw_tl(&[StreamedQuad::from_pos_and_size(0.0, 0.0, 1.0, 1.0)]);
    quads.set_color_attachment_texture();
    quads.blend(BlendType::None);
    quads.wrap(WrapType::Clamp);
    quads.set_stencil_mode(StencilMode::StencilNotPassed {
        clear_stencil: false,
    });
    if is_first {
        quads.set_color_mask(ColorMaskMode::WriteColorOnly);
    }
    drop(quads);

    graphics.next_switch_pass();

    if is_hori {
        render_blur_impl(
            graphics,
            false,
            blur_radius,
            (blur_mix_length - 1.0).max(1.0),
            blur_color,
            false,
        );
    } else if blur_mix_length > 1.0 {
        render_blur_impl(
            graphics,
            true,
            blur_radius,
            blur_mix_length - 1.0,
            blur_color,
            false,
        );
    }
}

pub fn render_blur(
    graphics: &mut Graphics,
    is_hori: bool,
    blur_radius: f32,
    blur_mix_length: f32,
    blur_color: &vec4,
) {
    let dynamic_viewport = graphics.canvas_handle.dynamic_viewport();
    graphics.canvas_handle.reset_window_viewport();
    render_blur_impl(
        graphics,
        is_hori,
        blur_radius,
        blur_mix_length,
        blur_color,
        true,
    );
    if let Some(dynamic_viewport) = dynamic_viewport {
        graphics.canvas_handle.update_window_viewport(
            dynamic_viewport.x,
            dynamic_viewport.y,
            dynamic_viewport.width,
            dynamic_viewport.height,
        );
    }
}

pub fn render_swapped_frame(graphics: &mut Graphics) {
    let dynamic_viewport = graphics.canvas_handle.dynamic_viewport();
    graphics.canvas_handle.reset_window_viewport();
    let mut quads = graphics.stream_handle.quads_begin();
    quads.set_colors_from_single(1.0, 1.0, 1.0, 1.0);
    quads.quads_set_subset_free(0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0);
    quads.map_canvas(0.0, 0.0, 1.0, 1.0);
    quads.quads_draw_tl(&[StreamedQuad::from_pos_and_size(0.0, 0.0, 1.0, 1.0)]);
    quads.set_color_attachment_texture();
    quads.blend(BlendType::None);
    quads.wrap(WrapType::Clamp);
    quads.set_stencil_mode(StencilMode::StencilNotPassed {
        clear_stencil: true,
    });
    drop(quads);
    if let Some(dynamic_viewport) = dynamic_viewport {
        graphics.canvas_handle.update_window_viewport(
            dynamic_viewport.x,
            dynamic_viewport.y,
            dynamic_viewport.width,
            dynamic_viewport.height,
        );
    }
}
