use egui_extras::TableRow;
use game_config::config::Config;

use crate::sort::{sortable_header, SortDir, TableSort};

/// table header
pub fn render(header: &mut TableRow<'_, '_>, config: &mut Config) {
    let sort: TableSort = config.storage("filter.sort");
    if sort.name.is_empty() {
        config.set_storage(
            "filter.sort",
            &TableSort {
                name: "Players".to_string(),
                sort_dir: SortDir::Desc,
            },
        );
    }
    sortable_header(
        header,
        "filter.sort",
        config,
        &["", "Name", "Type", "Map", "Players", "Ping"],
    );
}
