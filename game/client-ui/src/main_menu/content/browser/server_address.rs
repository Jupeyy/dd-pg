use std::net::SocketAddr;

use ui_base::{
    components::clearable_edit_field::clearable_edit_field, types::UiRenderPipe,
    utils::icon_font_plus_text,
};

use crate::main_menu::user_data::UserData;

/// server address input field
pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>) {
    ui.horizontal(|ui| {
        ui.label(icon_font_plus_text(ui, "\u{f233}", "- Address:"));
    });
    let mut cur_address: String = pipe
        .user_data
        .config
        .storage_opt::<SocketAddr>("server-addr")
        .map(|a| a.to_string())
        .unwrap_or_default();
    if clearable_edit_field(ui, &mut cur_address, Some(200.0), None)
        .map(|res| res.changed())
        .unwrap_or_default()
    {
        if let Ok(addr) = cur_address.parse::<SocketAddr>() {
            pipe.user_data.config.set_storage("server-addr", &addr);
        }
    }
}
