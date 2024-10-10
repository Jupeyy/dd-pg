use egui_extras::TableBody;
use ui_base::types::{UiRenderPipe, UiState};

use crate::main_menu::{favorite_player::FavoritePlayers, user_data::UserData};

/// server list frame (scrollable)
pub fn render(
    body: TableBody<'_>,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    favorites: &FavoritePlayers,
) {
    body.rows(25.0, favorites.len(), |row| {
        let row_index = row.index();

        let favorite = &favorites[row_index];

        super::entry::render(row, pipe, ui_state, favorite);
    });
}
