use client_ui::{
    connect::user_data::{ConnectMode, ConnectModes, UserData},
    events::UiEvents,
};
use ui_base::types::{UiRenderPipe, UiState};
use ui_traits::traits::UiPageInterface;

pub struct Connecting {}

impl Default for Connecting {
    fn default() -> Self {
        Self::new()
    }
}

impl Connecting {
    pub fn new() -> Self {
        Self {}
    }

    fn render_impl(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<()>,
        main_frame_only: bool,
    ) {
        client_ui::connect::main_frame::render(
            ui,
            &mut UiRenderPipe {
                cur_time: pipe.cur_time,
                user_data: &mut UserData {
                    mode: &ConnectMode::new(ConnectModes::Connecting),
                    config: &mut Default::default(),
                    events: &UiEvents::new(),
                },
            },
            main_frame_only,
        );
    }
}

impl UiPageInterface<()> for Connecting {
    fn has_blur(&self) -> bool {
        true
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<()>,
        _ui_state: &mut UiState,
    ) {
        self.render_impl(ui, pipe, true)
    }

    fn render(&mut self, ui: &mut egui::Ui, pipe: &mut UiRenderPipe<()>, _ui_state: &mut UiState) {
        self.render_impl(ui, pipe, false)
    }
}
