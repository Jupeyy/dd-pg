use client_types::server_browser::ServerBrowserServer;
use egui_extras::TableRow;
use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use ui_base::types::UIState;

/// single server list entry
pub fn render<'a, B: GraphicsBackendInterface>(
    mut row: TableRow<'_, '_>,
    row_index: usize,
    mut servers: impl Iterator<Item = &'a ServerBrowserServer>,
    _ui_state: &mut UIState,
    _graphics: &mut GraphicsBase<B>,
) {
    let server = servers.nth(row_index).unwrap();
    row.col(|ui| {
        ui.label("-");
    });
    row.col(|ui| {
        ui.label(&server.name);
    });
    row.col(|ui| {
        ui.label(&server.game_type);
    });
    row.col(|ui| {
        ui.label(&server.map);
    });
    row.col(|ui| {
        ui.label(&server.players.len().to_string());
    });
    row.col(|ui| {
        ui.label("EU");
    });
}
