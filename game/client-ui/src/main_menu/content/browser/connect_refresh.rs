use egui::Color32;

use ui_base::{style::default_style, types::UiRenderPipe, utils::icon_font_text_for_btn};

use crate::{events::UiEvent, main_menu::user_data::UserData};

/// connect & refresh button
pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>) {
    ui.horizontal(|ui| {
        let mut button_style = default_style();
        button_style.visuals.widgets.inactive.weak_bg_fill = Color32::DARK_GREEN;
        button_style.visuals.widgets.noninteractive.weak_bg_fill = Color32::DARK_GREEN;
        button_style.visuals.widgets.active.weak_bg_fill = Color32::DARK_GREEN;
        button_style.visuals.widgets.hovered.weak_bg_fill = Color32::DARK_GREEN;
        ui.set_style(button_style);

        let enter_clicked = ui.ctx().input(|i| i.key_pressed(egui::Key::Enter))
            && ui.ctx().memory(|m| m.focused().is_none());

        // connect
        if ui.button(icon_font_text_for_btn(ui, "\u{f2f6}")).clicked() || enter_clicked {
            if let Some(addr) = pipe.user_data.config.storage_opt("server-addr") {
                pipe.user_data.events.push(UiEvent::Connect {
                    addr,
                    cert_hash: pipe.user_data.config.storage("server-cert"),
                    rcon_secret: pipe.user_data.config.storage("rcon-secret"),
                });
            }
        }
    });
    // refresh
    if ui.button(icon_font_text_for_btn(ui, "\u{f2f9}")).clicked() {
        pipe.user_data.main_menu.refresh();
        let profiles = pipe.user_data.profiles.clone();
        pipe.user_data.profile_tasks.user_interactions.push(
            pipe.user_data
                .io
                .io_batcher
                .spawn(async move { profiles.user_interaction().await })
                .cancelable(),
        );
    }
}
