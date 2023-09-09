use std::sync::Arc;

use base_fs::{filesys::FileSystem, io_batcher::TokIOBatcher};
use client_render::containers::weapons::WeaponContainer;
use graphics_backend::types::Graphics;
use graphics_base::{
    quad_container::{GraphicsQuadContainerHandleInterface, QuadContainerIndex, SQuad},
    streaming::quad_scope_begin,
};
use graphics_types::rendering::State;
use math::math::vector::{dvec2, vec2};

use crate::client::components::{players::Players, render::get_sprite_scale_impl};

pub struct RenderCursorPipe<'a> {
    pub graphics: &'a mut Graphics,
    pub weapon_container: &'a mut WeaponContainer,
    pub fs: &'a Arc<FileSystem>,
    pub io_batcher: &'a TokIOBatcher,
    pub runtime_thread_pool: &'a Arc<rayon::ThreadPool>,
    pub mouse_cursor: dvec2,
}

pub struct RenderCursor {
    cursor_quad_container_index: QuadContainerIndex,
}

impl RenderCursor {
    pub fn new(graphics: &mut Graphics) -> Self {
        let cursor_quad_container_index =
            graphics.quad_container_handle.create_quad_container(false);

        let (scale_x, scale_y) = get_sprite_scale_impl(2, 2);

        graphics.quad_container_handle.quad_container_add_quads(
            &cursor_quad_container_index,
            &[SQuad::new()
                .from_width_and_height_centered(64.0 * scale_x, 64.0 * scale_y)
                .with_uv_from_points(&vec2::new(0.0, 0.0), &vec2::new(1.0, 1.0))],
        );

        graphics
            .quad_container_handle
            .quad_container_upload(&cursor_quad_container_index);

        Self {
            cursor_quad_container_index,
        }
    }

    pub fn render(&self, pipe: &mut RenderCursorPipe) {
        let cursor = pipe.weapon_container.get_or_default(
            "TODO:",
            &mut pipe.graphics,
            pipe.fs,
            pipe.io_batcher,
            pipe.runtime_thread_pool,
        );
        let mut state = State::default();
        Players::map_canvas_for_players(&pipe.graphics, &mut state, 0.0, 0.0, 1.0);

        let mut draw_scope = quad_scope_begin();
        draw_scope.set_state(&state);
        draw_scope.set_texture(&cursor.gun.cursor);
        draw_scope.set_colors_from_single(1.0, 1.0, 1.0, 1.0);

        let c = pipe.mouse_cursor;
        let c = vec2::new(c.x as f32, c.y as f32);

        pipe.graphics
            .quad_container_handle
            .render_quad_container_as_sprite(
                &self.cursor_quad_container_index,
                0,
                c.x,
                c.y,
                1.0,
                1.0,
                draw_scope,
            );
    }
}
