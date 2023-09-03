use std::sync::Arc;

use base_fs::{filesys::FileSystem, io_batcher::TokIOBatcher};
use graphics::graphics::QuadContainerRenderCount;
use graphics_backend::types::Graphics;
use graphics_base::{
    quad_container::{GraphicsQuadContainerHandleInterface, QuadContainerIndex, SQuad},
    streaming::quad_scope_begin,
};
use graphics_base_traits::traits::GraphicsSizeQuery;
use graphics_types::rendering::State;
use math::math::vector::vec2;

use crate::containers::hud::HudContainer;

pub struct RenderHudPipe<'a> {
    pub graphics: &'a mut Graphics,
    pub hud_container: &'a mut HudContainer,
    pub fs: &'a Arc<FileSystem>,
    pub io_batcher: &'a TokIOBatcher,
}

pub struct RenderHud {
    heart_shield_quad_container_index: QuadContainerIndex,

    heart_offset: usize,
    shield_offset: usize,
}

impl RenderHud {
    pub fn new(graphics: &mut Graphics) -> Self {
        let cursor_quad_container_index =
            graphics.quad_container_handle.create_quad_container(false);

        let mut quads: [SQuad; 10] = Default::default();

        let x = 5.0;
        let y = 5.0;
        // hearts
        quads.iter_mut().enumerate().for_each(|(index, q)| {
            *q = SQuad::new()
                .from_rect(x + index as f32 * 12.0, y, 12.0, 12.0)
                .with_uv_from_points(&vec2::new(0.0, 0.0), &vec2::new(1.0, 1.0));
        });

        let heart_offset = graphics
            .quad_container_handle
            .quad_container_add_quads(&cursor_quad_container_index, &quads);

        // shields
        quads.iter_mut().enumerate().for_each(|(index, q)| {
            *q = SQuad::new()
                .from_rect(x + index as f32 * 12.0, y + 12.0, 12.0, 12.0)
                .with_uv_from_points(&vec2::new(0.0, 0.0), &vec2::new(1.0, 1.0));
        });

        let shield_offset = graphics
            .quad_container_handle
            .quad_container_add_quads(&cursor_quad_container_index, &quads);

        graphics
            .quad_container_handle
            .quad_container_upload(&cursor_quad_container_index);

        Self {
            heart_shield_quad_container_index: cursor_quad_container_index,
            heart_offset,
            shield_offset,
        }
    }

    pub fn render(&self, pipe: &mut RenderHudPipe) {
        let hud = pipe.hud_container.get_or_default(
            "TODO:",
            &mut pipe.graphics,
            &pipe.fs,
            &pipe.io_batcher,
        );
        let mut state = State::default();
        state.map_canvas(0.0, 0.0, 300.0 * pipe.graphics.canvas_aspect(), 300.0);

        let mut draw_scope = quad_scope_begin();
        draw_scope.set_state(&state);
        draw_scope.set_colors_from_single(1.0, 1.0, 1.0, 1.0);

        // render heart
        draw_scope.set_texture(&hud.heart);
        pipe.graphics.quad_container_handle.render_quad_container(
            &self.heart_shield_quad_container_index,
            self.heart_offset,
            &QuadContainerRenderCount::Count(10),
            0.0,
            0.0,
            1.0,
            1.0,
            draw_scope.clone(),
        );

        // render shields
        draw_scope.set_texture(&hud.shield);
        pipe.graphics.quad_container_handle.render_quad_container(
            &self.heart_shield_quad_container_index,
            self.shield_offset,
            &QuadContainerRenderCount::Count(10),
            0.0,
            0.0,
            1.0,
            1.0,
            draw_scope,
        );
    }
}
