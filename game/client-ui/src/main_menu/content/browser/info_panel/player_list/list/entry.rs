use egui_extras::TableRow;
use shared_base::server_browser::ServerBrowserPlayer;

use ui_base::types::UiState;

/// single server list entry
pub fn render(
    mut row: TableRow<'_, '_>,
    row_index: usize,
    player: &ServerBrowserPlayer,
    _ui_state: &mut UiState,
) {
    row.col(|ui| {
        ui.label(&player.score);
    });
    row.col(|ui| {
        ui.label(&player.name);
    });
    row.col(|ui| {
        ui.label(format!("flag: {}", player.country));
    });
}
