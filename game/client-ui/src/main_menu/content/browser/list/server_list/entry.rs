use client_types::server_browser::ServerBrowserServer;
use egui_extras::TableRow;

use ui_base::{types::UiState, utils::icon_font_text};

/// single server list entry
pub fn render(
    mut row: TableRow<'_, '_>,
    server: &ServerBrowserServer,
    _ui_state: &mut UiState,
) -> bool {
    let mut clicked = false;
    clicked |= row
        .col(|ui| {
            clicked |= if server.info.passworded {
                ui.label(icon_font_text(ui, "\u{f023}"))
            } else {
                ui.label("")
            }
            .clicked();
        })
        .1
        .clicked();
    clicked |= row
        .col(|ui| {
            clicked |= ui.label(&server.info.name).clicked();
        })
        .1
        .clicked();
    clicked |= row
        .col(|ui| {
            clicked |= ui.label(&server.info.game_type).clicked();
        })
        .1
        .clicked();
    clicked |= row
        .col(|ui| {
            clicked |= ui.label(&server.info.map.name).clicked();
        })
        .1
        .clicked();
    clicked |= row
        .col(|ui| {
            clicked |= ui.label(server.info.players.len().to_string()).clicked();
        })
        .1
        .clicked();
    clicked |= row
        .col(|ui| {
            clicked |= ui.label("EU").clicked();
        })
        .1
        .clicked();
    clicked
}
