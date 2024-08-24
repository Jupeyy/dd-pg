use egui::Layout;
use ui_base::types::{UiRenderPipe, UiState};

use crate::main_menu::user_data::UserData;

pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>, ui_state: &mut UiState) {
    ui.with_layout(Layout::top_down(egui::Align::Min), |ui| {
        super::themes::list::render(ui, pipe, ui_state);
    });
}
