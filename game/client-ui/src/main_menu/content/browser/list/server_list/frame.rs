use egui_extras::TableBody;
use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use ui_base::types::{UIPipe, UIState};

use crate::main_menu::user_data::UserData;

/// server list frame (scrollable)
pub fn render<B: GraphicsBackendInterface>(
    body: TableBody<'_>,
    pipe: &mut UIPipe<UserData>,
    ui_state: &mut UIState,
    graphics: &mut GraphicsBase<B>,
) {
    let servers_filtered = pipe.user_data.browser_data.servers.iter().filter(|server| {
        server
            .map
            .to_lowercase()
            .contains(&pipe.user_data.browser_data.filter.search.to_lowercase())
            || server
                .name
                .to_lowercase()
                .contains(&pipe.user_data.browser_data.filter.search.to_lowercase())
    });
    body.rows(25.0, servers_filtered.clone().count(), |row_index, row| {
        super::entry::render(row, row_index, servers_filtered.clone(), ui_state, graphics);
    });
}
