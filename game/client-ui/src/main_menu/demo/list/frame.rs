use std::path::PathBuf;

use egui_extras::TableBody;
use game_config::config::Config;

use ui_base::types::UiRenderPipe;

use crate::{
    events::UiEvent,
    main_menu::{
        demo_list::{DemoList, DemoListEntry},
        user_data::UserData,
    },
    sort::{SortDir, TableSort},
};

fn demos_filtered<'a>(
    demos: &'a DemoList,
    config: &mut Config,
) -> impl Iterator<Item = &'a DemoListEntry> {
    let search = config.storage_entry("demo.search").clone();
    demos.iter().filter(move |demo| {
        match demo {
            DemoListEntry::File { name, .. } => name,
            DemoListEntry::Directory { name } => name,
        }
        .to_lowercase()
        .contains(&search.to_lowercase())
    })
}

fn demos_sorted(demos: &mut [&DemoListEntry], config: &Config) {
    let sort: TableSort = config.storage("demo.sort");
    demos.sort_by(|d1, d2| match d1 {
        DemoListEntry::File {
            date: date1,
            name: name1,
        } => match d2 {
            DemoListEntry::File {
                date: date2,
                name: name2,
            } => match sort.name.as_str() {
                "Name" => match sort.sort_dir {
                    SortDir::Asc => name1.cmp(name2),
                    SortDir::Desc => name2.cmp(name1),
                },
                "Date" => match sort.sort_dir {
                    SortDir::Asc => date1.cmp(date2),
                    SortDir::Desc => date2.cmp(date1),
                },
                _ => date1.cmp(date2),
            },
            DemoListEntry::Directory { .. } => std::cmp::Ordering::Less,
        },
        DemoListEntry::Directory { name: name1 } => match d2 {
            DemoListEntry::File { .. } => std::cmp::Ordering::Greater,
            // dicts always compare name DESC
            DemoListEntry::Directory { name: name2 } => name2.cmp(name1),
        },
    });
}

/// demo list frame (scrollable)
pub fn render(mut body: TableBody<'_>, pipe: &mut UiRenderPipe<UserData>) {
    let mut demos_filtered: Vec<_> =
        demos_filtered(pipe.user_data.demos, pipe.user_data.config).collect();
    demos_sorted(&mut demos_filtered, pipe.user_data.config);

    let select_prev = body
        .ui_mut()
        .ctx()
        .input(|i| i.key_pressed(egui::Key::ArrowUp))
        && body.ui_mut().ctx().memory(|m| m.focused().is_none());
    let select_next = body
        .ui_mut()
        .ctx()
        .input(|i| i.key_pressed(egui::Key::ArrowDown))
        && body.ui_mut().ctx().memory(|m| m.focused().is_none());
    let select_first = body
        .ui_mut()
        .ctx()
        .input(|i| i.key_pressed(egui::Key::PageUp))
        && body.ui_mut().ctx().memory(|m| m.focused().is_none());
    let select_last = body
        .ui_mut()
        .ctx()
        .input(|i| i.key_pressed(egui::Key::PageDown))
        && body.ui_mut().ctx().memory(|m| m.focused().is_none());

    let selected_demo: String = pipe.user_data.config.storage::<String>("selected-demo");

    body.rows(30.0, demos_filtered.len(), |mut row| {
        let row_index = row.index();

        let demo = &demos_filtered[row_index];

        let select_index = if select_prev {
            Some(row_index + 1)
        } else if select_next {
            Some(row_index.saturating_sub(1))
        } else if select_first {
            Some(0)
        } else if select_last {
            Some(demos_filtered.len().saturating_sub(1))
        } else {
            None
        };

        let is_selected = match demo {
            DemoListEntry::File { name, .. } => name,
            DemoListEntry::Directory { name } => name,
        }
        .eq(&selected_demo);
        row.set_selected(is_selected);
        let response = super::entry::render(row, demo);

        let clicked = response.clicked()
            || select_index
                .and_then(|index| demos_filtered.get(index).copied())
                .is_some_and(|s| {
                    match s {
                        DemoListEntry::File { name, .. } => name,
                        DemoListEntry::Directory { name } => name,
                    }
                    .eq(&selected_demo)
                });

        // extra check here, bcs the demo might be changed by keyboard
        if clicked {
            let file = match demo {
                DemoListEntry::File { name, .. } => name,
                DemoListEntry::Directory { name } => name,
            };
            pipe.user_data.config.set_storage("selected-demo", &file);
            let cur_path: String = pipe.user_data.config.storage("demo-path");
            let cur_path: PathBuf = cur_path.into();
            let file_path = cur_path.join(file);
            pipe.user_data.main_menu.refresh_demo_list(&file_path);
        }
        if response.double_clicked() {
            let cur_path: String = pipe.user_data.config.storage("demo-path");
            let cur_path: PathBuf = cur_path.into();

            match demo {
                DemoListEntry::Directory { name } => {
                    let new_path = cur_path.join(name);
                    pipe.user_data
                        .config
                        .set_storage("demo-path", &new_path.to_string_lossy());
                    pipe.user_data
                        .main_menu
                        .refresh_demo_list(new_path.as_ref());
                }
                DemoListEntry::File { name, .. } => {
                    let new_path = cur_path.join(name);
                    pipe.user_data
                        .events
                        .push(UiEvent::PlayDemo { name: new_path });
                }
            }
        }
    });
}
