use graphics_backend_traits::traits::GraphicsBackendInterface;
use graphics_base::streaming::{DrawScopeImpl, GraphicsStreamHandleInterface};
use graphics_types::{rendering::RenderMode, types::StreamedQuad};
use math::math::vector::vec4;

use crate::graphics::GraphicsBase;

pub fn render_blur_impl<B>(
    graphics: &mut GraphicsBase<B>,
    is_hori: bool,
    blur_radius: f32,
    blur_color: &vec4,
    depth: usize,
) where
    B: Clone + GraphicsBackendInterface + 'static,
{
    let mut quads = graphics.quads_begin();
    quads.set_colors_from_single(1.0, 0.0, 0.0, 1.0);
    quads.quads_set_subset_free(0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0);
    quads.map_canvas(0.0, 0.0, 1.0, 1.0);
    quads.quads_draw_tl(&[StreamedQuad::from_pos_and_size(0.25, 0.25, 0.5, 0.75)]);
    quads.set_render_mode(RenderMode::AsPassTransition {
        blur_radius: blur_radius,
        blur_horizontal: is_hori,
        blur_color: if !is_hori && depth == 0 {
            *blur_color
        } else {
            vec4::new(1.0, 1.0, 1.0, 0.0)
        },
    });
    drop(quads);
    let mut quads = graphics.quads_begin();
    quads.set_colors_from_single(1.0, 0.0, 0.0, 1.0);
    quads.quads_set_subset_free(0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0);
    quads.map_canvas(0.0, 0.0, 1.0, 1.0);
    quads.quads_draw_tl(&[StreamedQuad::from_pos_and_size(0.25, 0.25, 0.5, 0.75)]);
    quads.set_render_mode(RenderMode::StencilNotPassed {
        clear_stencil: false,
    });
    drop(quads);

    graphics.next_switch_pass();

    if is_hori {
        render_blur_impl(graphics, false, blur_radius, blur_color, depth);
    } else if depth > 0 {
        render_blur_impl(graphics, true, blur_radius, blur_color, depth - 1);
    }
}

pub fn render_blur<B>(
    graphics: &mut GraphicsBase<B>,
    is_hori: bool,
    blur_radius: f32,
    blur_color: &vec4,
) where
    B: Clone + GraphicsBackendInterface + 'static,
{
    render_blur_impl(graphics, is_hori, blur_radius, blur_color, 40)
}

pub fn render_swapped_frame<B>(graphics: &mut GraphicsBase<B>)
where
    B: Clone + GraphicsBackendInterface + 'static,
{
    let mut quads = graphics.quads_begin();
    quads.set_colors_from_single(1.0, 0.0, 0.0, 1.0);
    quads.quads_set_subset_free(0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0);
    quads.map_canvas(0.0, 0.0, 1.0, 1.0);
    quads.quads_draw_tl(&[StreamedQuad::from_pos_and_size(1.0, 1.0, 1.0, 1.0)]);
    quads.set_render_mode(RenderMode::StencilNotPassed {
        clear_stencil: true,
    });
    drop(quads);
}
