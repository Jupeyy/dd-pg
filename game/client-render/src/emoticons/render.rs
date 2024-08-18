use client_containers::{container::ContainerKey, emoticons::EmoticonsContainer};
use client_render_base::render::canvas_mapping::CanvasMappingIngame;
use game_interface::types::emoticons::EmoticonType;
use graphics::{
    graphics::graphics::Graphics, handles::quad_container::quad_container::QuadContainer,
    quad_container::Quad, streaming::quad_scope_begin,
};
use graphics_types::rendering::State;
use math::math::vector::{ubvec4, vec2};

pub struct RenderEmoticonPipe<'a> {
    pub emoticon_container: &'a mut EmoticonsContainer,

    pub pos: vec2,
    pub emoticon: EmoticonType,
}

pub struct RenderEmoticon {
    quad_container: QuadContainer,
    canvas_mapping: CanvasMappingIngame,
}

impl RenderEmoticon {
    pub fn new(graphics: &Graphics) -> Self {
        let quads: Vec<Quad> = vec![Quad::new()
            .from_size_centered(64.0)
            .with_color(&ubvec4::new(255, 255, 255, 255))
            .with_uv_from_points(&vec2::new(0.0, 0.0), &vec2::new(1.0, 1.0))];

        let quad_container = graphics.quad_container_handle.create_quad_container(quads);

        Self {
            quad_container,
            canvas_mapping: CanvasMappingIngame::new(graphics),
        }
    }

    pub fn render(&self, pipe: &mut RenderEmoticonPipe) {
        let mut state = State::new();
        self.canvas_mapping
            .map_canvas_for_ingame_items(&mut state, 0.0, 0.0, 1.0);

        let emoticon = pipe
            .emoticon_container
            .get_or_default::<ContainerKey>(&"TODO".try_into().unwrap());
        let texture = &emoticon.emoticons[pipe.emoticon as usize];

        let mut quad_scope = quad_scope_begin();
        quad_scope.set_state(&state);

        self.quad_container.render_quad_container_as_sprite(
            0,
            0.0,
            0.0,
            1.0,
            1.0,
            quad_scope,
            texture.into(),
        );
    }
}
