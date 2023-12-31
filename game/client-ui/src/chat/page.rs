use graphics::graphics::Graphics;
use ui_base::{
    style::default_style,
    types::{UIPipe, UIState},
};
use ui_traits::traits::UIRenderCallbackFunc;

use super::{main_frame, user_data::UserData};

pub struct ChatUI {}

impl ChatUI {
    pub fn new() -> Self {
        Self {}
    }

    pub fn set_style(ui: &mut egui::Ui) {
        ui.set_style(default_style());
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
        graphics: &mut Graphics,
    ) {
        Self::set_style(ui);
        main_frame::render(ui, pipe, ui_state, graphics, true)
    }

    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<UserData>,
        ui_state: &mut UIState,
        graphics: &mut Graphics,
    ) {
        Self::set_style(ui);
        main_frame::render(ui, pipe, ui_state, graphics, false)
    }
}
