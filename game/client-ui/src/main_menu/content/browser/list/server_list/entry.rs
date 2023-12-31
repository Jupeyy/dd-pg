use client_types::server_browser::ServerBrowserServer;
use egui_extras::TableRow;
use graphics::graphics::Graphics;
use ui_base::types::UIState;

/// single server list entry
pub fn render<'a>(
    mut row: TableRow<'_, '_>,
    row_index: usize,
    mut servers: impl Iterator<Item = &'a ServerBrowserServer>,
    _ui_state: &mut UIState,
    _graphics: &mut Graphics,
) -> bool {
    let mut clicked = false;
    let server = servers.nth(row_index).unwrap();
    clicked |= row
        .col(|ui| {
            ui.label("-");
        })
        .1
        .clicked();
    clicked |= row
        .col(|ui| {
            ui.label(&server.info.name);
        })
        .1
        .clicked();
    clicked |= row
        .col(|ui| {
            ui.label(&server.info.game_type);
        })
        .1
        .clicked();
    clicked |= row
        .col(|ui| {
            ui.label(&server.info.map);
        })
        .1
        .clicked();
    clicked |= row
        .col(|ui| {
            ui.label(&server.info.players.len().to_string());
        })
        .1
        .clicked();
    clicked |= row
        .col(|ui| {
            ui.label("EU");
        })
        .1
        .clicked();
    clicked
}
