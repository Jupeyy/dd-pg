use egui_extras::TableBody;
use shared_base::server_browser::ServerBrowserServer;

use ui_base::types::UiState;

/// server list frame (scrollable)
pub fn render(body: TableBody<'_>, ui_state: &mut UiState, cur_server: &ServerBrowserServer) {
    body.rows(25.0, cur_server.info.players.len(), |row| {
        let row_index = row.index();
        let player = &cur_server.info.players[row_index];
        super::entry::render(row, row_index, player, ui_state);
    });
}
