use ui_base::types::{UIPipe, UIState};
use ui_traits::traits::UIRenderCallbackFunc;

use super::{main_frame, user_data::UserData};

pub struct ChatUI {}

impl ChatUI {
    pub fn new() -> Self {
        Self {}
    }
}

impl<'a> UIRenderCallbackFunc<UserData<'a>> for ChatUI {
    fn has_blur(&self) -> bool {
        false
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<UserData>,
        ui_state: &mut UIState,
    ) {
        main_frame::render(ui, pipe, ui_state, true)
    }

    fn render(&mut self, ui: &mut egui::Ui, pipe: &mut UIPipe<UserData>, ui_state: &mut UIState) {
        main_frame::render(ui, pipe, ui_state, false)
    }
}
