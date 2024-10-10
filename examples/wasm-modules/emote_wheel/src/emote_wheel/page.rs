use api_ui_game::render::{create_emoticons_container, create_skin_container};
use client_containers::{emoticons::EmoticonsContainer, skins::SkinContainer};
use client_render_base::render::tee::RenderTee;
use graphics::{
    graphics::graphics::Graphics,
    handles::{canvas::canvas::GraphicsCanvasHandle, stream::stream::GraphicsStreamHandle},
};
use ui_base::types::{UiRenderPipe, UiState};
use ui_traits::traits::UiPageInterface;

pub struct EmoteWheelPage {
    canvas_handle: GraphicsCanvasHandle,
    stream_handle: GraphicsStreamHandle,
    skin_container: SkinContainer,
    emoticons_container: EmoticonsContainer,
    render_tee: RenderTee,
}

impl EmoteWheelPage {
    pub fn new(graphics: &Graphics) -> Self {
        Self {
            canvas_handle: graphics.canvas_handle.clone(),
            stream_handle: graphics.stream_handle.clone(),
            skin_container: create_skin_container(),
            emoticons_container: create_emoticons_container(),
            render_tee: RenderTee::new(graphics),
        }
    }

    fn render_impl(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<()>,
        ui_state: &mut UiState,
        main_frame_only: bool,
    ) {
        client_ui::emote_wheel::main_frame::render(
            ui,
            &mut UiRenderPipe::new(
                pipe.cur_time,
                &mut client_ui::emote_wheel::user_data::UserData {
                    events: &mut Default::default(),
                    canvas_handle: &self.canvas_handle,
                    stream_handle: &self.stream_handle,
                    skin_container: &mut self.skin_container,
                    emoticons_container: &mut self.emoticons_container,
                    render_tee: &self.render_tee,
                    emoticon: &Default::default(),
                    skin: &Default::default(),
                    skin_info: &None,
                },
            ),
            ui_state,
            main_frame_only,
        );
    }
}

impl UiPageInterface<()> for EmoteWheelPage {
    fn has_blur(&self) -> bool {
        true
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<()>,
        ui_state: &mut UiState,
    ) {
        self.render_impl(ui, pipe, ui_state, true)
    }

    fn render(&mut self, ui: &mut egui::Ui, pipe: &mut UiRenderPipe<()>, ui_state: &mut UiState) {
        self.render_impl(ui, pipe, ui_state, false)
    }
}
