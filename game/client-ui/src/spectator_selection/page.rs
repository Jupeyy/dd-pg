use ui_base::types::{UiRenderPipe, UiState};
use ui_traits::traits::UiPageInterface;

use super::{main_frame, user_data::UserData};

pub struct SpectatorSelectionUi {}

impl Default for SpectatorSelectionUi {
    fn default() -> Self {
        Self::new()
    }
}

impl SpectatorSelectionUi {
    pub fn new() -> Self {
        Self {}
    }
}

impl UiPageInterface<UserData<'_>> for SpectatorSelectionUi {
    fn has_blur(&self) -> bool {
        false
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<UserData>,
        ui_state: &mut UiState,
    ) {
        main_frame::render(ui, pipe, ui_state, true)
    }

    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<UserData>,
        ui_state: &mut UiState,
    ) {
        main_frame::render(ui, pipe, ui_state, false)
    }
}
