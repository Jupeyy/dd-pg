use egui::Color32;

use ui_base::{
    components::menu_top_button::text_icon,
    style::default_style,
    types::{UIPipe, UIState},
};

use crate::{events::UiEvent, main_menu::user_data::UserData};

/// connect & refresh button
pub fn render(ui: &mut egui::Ui, pipe: &mut UIPipe<UserData>, _ui_state: &mut UIState) {
    ui.horizontal(|ui| {
        let mut button_style = default_style();
        button_style.visuals.widgets.inactive.weak_bg_fill = Color32::DARK_GREEN;
        button_style.visuals.widgets.noninteractive.weak_bg_fill = Color32::DARK_GREEN;
        button_style.visuals.widgets.active.weak_bg_fill = Color32::DARK_GREEN;
        button_style.visuals.widgets.hovered.weak_bg_fill = Color32::DARK_GREEN;
        ui.set_style(button_style);
        let text = text_icon(ui, "\u{f2f6}");
        if ui.button(text).clicked() {
            pipe.user_data.events.push(UiEvent::Connect {
                addr: pipe
                    .user_data
                    .browser_data
                    .cur_address
                    .clone()
                    .parse()
                    .unwrap(),
            });
        }
    });
    let text = text_icon(ui, "\u{f2f9}");
    if ui.button(text).clicked() {
        pipe.user_data.main_menu.refresh();
    }
}
