use std::sync::Arc;

use base_io::io::IO;
use client_containers::weapons::WeaponContainer;
use client_render_base::render::canvas_mapping::map_canvas_for_ingame_items;
use graphics::{
    graphics::Graphics, handles::quad_container::QuadContainer, quad_container::Quad,
    streaming::quad_scope_begin,
};
use graphics_types::rendering::State;
use math::math::vector::{dvec2, vec2};

use crate::client::components::render::get_sprite_scale_impl;

pub struct RenderCursorPipe<'a> {
    pub graphics: &'a mut Graphics,
    pub weapon_container: &'a mut WeaponContainer,
    pub io: &'a IO,
    pub runtime_thread_pool: &'a Arc<rayon::ThreadPool>,
    pub mouse_cursor: dvec2,
}

pub struct RenderCursor {
    cursor_quad_container: QuadContainer,
}

impl RenderCursor {
    pub fn new(graphics: &mut Graphics) -> Self {
        let (scale_x, scale_y) = get_sprite_scale_impl(2, 2);

        let cursor_quad_container = graphics.quad_container_handle.create_quad_container(
            [Quad::new()
                .from_width_and_height_centered(64.0 * scale_x, 64.0 * scale_y)
                .with_uv_from_points(&vec2::new(0.0, 0.0), &vec2::new(1.0, 1.0))]
            .into(),
        );

        Self {
            cursor_quad_container,
        }
    }

    pub fn render(&self, pipe: &mut RenderCursorPipe) {
        let cursor = pipe.weapon_container.get_or_default("TODO:");
        let mut state = State::default();
        map_canvas_for_ingame_items(pipe.graphics, &mut state, 0.0, 0.0, 1.0);

        let mut draw_scope = quad_scope_begin();
        draw_scope.set_state(&state);
        draw_scope.set_texture(&cursor.gun.cursor);
        draw_scope.set_colors_from_single(1.0, 1.0, 1.0, 1.0);

        let c = pipe.mouse_cursor;
        let c = vec2::new(c.x as f32, c.y as f32);

        self.cursor_quad_container
            .render_quad_container_as_sprite(0, c.x, c.y, 1.0, 1.0, draw_scope);
    }
}
