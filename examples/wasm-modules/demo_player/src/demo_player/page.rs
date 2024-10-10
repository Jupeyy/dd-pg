use std::time::Duration;

use client_ui::demo_player::user_data::{DemoViewerEvent, DemoViewerUiState};
use graphics::{
    graphics::graphics::Graphics,
    handles::{canvas::canvas::GraphicsCanvasHandle, stream::stream::GraphicsStreamHandle},
};
use ui_base::types::{UiRenderPipe, UiState};
use ui_traits::traits::UiPageInterface;

pub struct DemoPlayerPage {
    canvas_handle: GraphicsCanvasHandle,
    stream_handle: GraphicsStreamHandle,

    paused: bool,
    state: DemoViewerUiState,
}

impl DemoPlayerPage {
    pub fn new(graphics: &Graphics) -> Self {
        Self {
            canvas_handle: graphics.canvas_handle.clone(),
            stream_handle: graphics.stream_handle.clone(),

            paused: false,
            state: Default::default(),
        }
    }

    fn render_impl(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<()>,
        _ui_state: &mut UiState,
        main_frame_only: bool,
    ) {
        let mut events = Default::default();
        client_ui::demo_player::main_frame::render(
            ui,
            &mut UiRenderPipe::new(
                pipe.cur_time,
                &mut client_ui::demo_player::user_data::UserData {
                    canvas_handle: &self.canvas_handle,
                    stream_handle: &self.stream_handle,
                    is_paused: &self.paused,
                    cur_duration: &mut Duration::from_secs_f32(2.5),
                    max_duration: &Duration::from_secs_f32(5.5),
                    events: &mut events,
                    speed: &Default::default(),
                    state: &mut self.state,
                    name: "example_demo",
                },
            ),
            main_frame_only,
        );
        for event in events {
            if let DemoViewerEvent::ResumeToggle = event {
                self.paused = !self.paused;
            }
        }
    }
}

impl UiPageInterface<()> for DemoPlayerPage {
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
