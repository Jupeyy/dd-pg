use ui_base::types::{UiRenderPipe, UiState};
use ui_traits::traits::UiPageInterface;

use super::{main_frame, user_data::UserData};

pub struct ConsoleUi {}

impl ConsoleUi {
    pub fn new() -> Self {
        Self {}
    }
}

impl<'a> UiPageInterface<UserData<'a>> for ConsoleUi {
    fn has_blur(&self) -> bool {
        true
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<UserData<'a>>,
        ui_state: &mut UiState,
    ) {
        main_frame::render(ui, pipe, ui_state, true);
    }

    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<UserData<'a>>,
        ui_state: &mut UiState,
    ) {
        main_frame::render(ui, pipe, ui_state, false)
    }
}
