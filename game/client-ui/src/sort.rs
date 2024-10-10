use egui_extras::TableRow;
use game_config::config::Config;
use serde::{Deserialize, Serialize};
use ui_base::utils::icon_font_text_for_text;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub enum SortDir {
    #[default]
    Asc,
    Desc,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TableSort {
    pub name: String,
    pub sort_dir: SortDir,
}

pub fn sortable_header(
    header: &mut TableRow<'_, '_>,
    storage_name: &str,
    config: &mut Config,
    names: &[&str],
) {
    let sort: TableSort = config.storage(storage_name);
    let mut item = |name: &str| {
        let is_selected = name == sort.name;
        header.set_selected(is_selected);
        let mut clicked = false;
        clicked |= header
            .col(|ui| {
                ui.horizontal(|ui| {
                    clicked |= ui.strong(name).clicked();
                    if is_selected {
                        clicked |= ui
                            .strong(icon_font_text_for_text(
                                ui,
                                match sort.sort_dir {
                                    SortDir::Asc => "\u{f0de}",
                                    SortDir::Desc => "\u{f0dd}",
                                },
                            ))
                            .clicked();
                    }
                });
            })
            .1
            .clicked();

        if clicked {
            config.set_storage::<TableSort>(
                storage_name,
                &TableSort {
                    name: name.to_string(),
                    sort_dir: if is_selected {
                        match sort.sort_dir {
                            SortDir::Asc => SortDir::Desc,
                            SortDir::Desc => SortDir::Asc,
                        }
                    } else {
                        Default::default()
                    },
                },
            );
        }
    };

    for name in names {
        item(name);
    }
}
