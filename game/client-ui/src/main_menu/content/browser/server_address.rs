use egui::{FontId, TextFormat};

use ui_base::{
    components::{clearable_edit_field::clearable_edit_field, menu_top_button::text_icon},
    types::{UiRenderPipe, UiState},
};

use crate::main_menu::user_data::UserData;

/// server address input field
pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>, _ui_state: &mut UiState) {
    ui.horizontal(|ui| {
        let mut text = text_icon(ui, "\u{f233}");
        text.append(
            "- Address:",
            4.0,
            TextFormat::simple(FontId::default(), ui.style().visuals.text_color()),
        );
        ui.label(text);
    });
    pipe.user_data.browser_data.cur_cert_hash = pipe
        .user_data
        .config
        .engine
        .ui
        .storage
        .get("server-cert")
        .and_then(|cert| base::hash::decode_hash(cert));
    pipe.user_data.browser_data.cur_address = pipe
        .user_data
        .config
        .engine
        .ui
        .storage
        .get("server-addr")
        .cloned()
        .unwrap_or_default();
    if clearable_edit_field(
        ui,
        &mut pipe.user_data.browser_data.cur_address,
        Some(200.0),
    )
    .map(|res| res.changed())
    .unwrap_or_default()
    {
        pipe.user_data.config.engine.ui.storage.insert(
            "server-addr".to_string(),
            pipe.user_data.browser_data.cur_address.clone(),
        );
    }
}
