use graphics::graphics::Graphics;
use ui_base::types::{UIPipe, UIState};
use ui_traits::traits::UIRenderCallbackFunc;

use super::{main_frame, user_data::UserData};

pub struct ConsoleUI {}

impl ConsoleUI {
    pub fn new() -> Self {
        Self {}
    }
}

impl<'a> UIRenderCallbackFunc<UserData<'a>> for ConsoleUI {
    fn has_blur(&self) -> bool {
        true
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<UserData<'a>>,
        ui_state: &mut UIState,
        graphics: &mut Graphics,
    ) {
        main_frame::render(ui, pipe, ui_state, graphics, true);
    }

    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<UserData<'a>>,
        ui_state: &mut UIState,
        graphics: &mut Graphics,
    ) {
        main_frame::render(ui, pipe, ui_state, graphics, false)
    }
}
