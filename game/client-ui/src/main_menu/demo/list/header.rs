use crate::sort::sortable_header;
use egui_extras::TableRow;
use game_config::config::Config;

/// table header
pub fn render(header: &mut TableRow<'_, '_>, config: &mut Config) {
    sortable_header(header, "demo.sort", config, &["", "Name", "Date"])
}
