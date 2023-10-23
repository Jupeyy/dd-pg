use std::sync::Arc;

use base_io::io::IO;
use client_containers::emoticons::{EmoticonType, EmoticonsContainer};
use client_render_base::render::canvas_mapping::map_canvas_for_ingame_items;
use graphics_backend::types::Graphics;
use graphics_base::{
    quad_container::{GraphicsQuadContainerHandleInterface, QuadContainerIndex, SQuad},
    streaming::quad_scope_begin,
};
use graphics_types::rendering::State;
use math::math::vector::{ubvec4, vec2};

pub struct RenderEmoticonPipe<'a> {
    pub graphics: &'a mut Graphics,
    pub emoticon_container: &'a mut EmoticonsContainer,

    pub runtime_thread_pool: &'a Arc<rayon::ThreadPool>,
    pub io: &'a IO,

    pub pos: vec2,
    pub emoticon: EmoticonType,
}

pub struct RenderEmoticon {
    quad_container_index: QuadContainerIndex,
}

impl RenderEmoticon {
    pub fn new(graphics: &mut Graphics) -> Self {
        let quad_container_index = graphics.quad_container_handle.create_quad_container(false);

        let quad = SQuad::new()
            .from_size_centered(64.0)
            .with_color(&ubvec4::new(255, 255, 255, 255))
            .with_uv_from_points(&vec2::new(0.0, 0.0), &vec2::new(1.0, 1.0));

        graphics
            .quad_container_handle
            .quad_container_add_quads(&quad_container_index, &[quad]);

        graphics
            .quad_container_handle
            .quad_container_upload(&quad_container_index);

        Self {
            quad_container_index,
        }
    }

    pub fn render(&self, pipe: &mut RenderEmoticonPipe) {
        let mut state = State::new();
        map_canvas_for_ingame_items(pipe.graphics, &mut state, 0.0, 0.0, 1.0);

        let emoticon = pipe
            .emoticon_container
            .get_or_default("TODO:", pipe.graphics);
        state.set_texture(&emoticon.emoticons[pipe.emoticon as usize]);

        let mut quad_scope = quad_scope_begin();
        quad_scope.set_state(&state);

        pipe.graphics
            .quad_container_handle
            .render_quad_container_as_sprite(
                &self.quad_container_index,
                0,
                0.0,
                0.0,
                1.0,
                1.0,
                quad_scope,
            );
    }
}
