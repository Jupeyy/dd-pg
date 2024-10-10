use ui_base::types::{UiRenderPipe, UiState};
use ui_traits::traits::UiPageInterface;

use super::{main_frame, user_data::UserData};

pub struct DemoPlayerUi {}

impl Default for DemoPlayerUi {
    fn default() -> Self {
        Self::new()
    }
}

impl DemoPlayerUi {
    pub fn new() -> Self {
        Self {}
    }
}

impl<'a> UiPageInterface<UserData<'a>> for DemoPlayerUi {
    fn has_blur(&self) -> bool {
        true
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<UserData>,
        _ui_state: &mut UiState,
    ) {
        main_frame::render(ui, pipe, true)
    }

    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<UserData>,
        _ui_state: &mut UiState,
    ) {
        main_frame::render(ui, pipe, false)
    }
}
