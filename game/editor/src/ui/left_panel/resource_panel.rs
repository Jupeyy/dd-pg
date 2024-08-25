use std::path::Path;

use base_io::io::Io;
use egui::{vec2, Button, Layout};
use egui_file_dialog::{DialogMode, DialogState};
use map::skeleton::resources::MapResourceRefSkeleton;

use crate::{
    client::EditorClient, fs::read_file_editor, map::EditorGroupPanelResources,
    ui::utils::icon_font_text,
};

pub fn render<F, R, U>(
    ui: &mut egui::Ui,
    main_frame_only: bool,
    client: &mut EditorClient,
    resources: &mut Vec<MapResourceRefSkeleton<U>>,
    panel_data: &mut EditorGroupPanelResources,
    io: &Io,
    load_resource: F,
    rem_resource: R,
) where
    F: Fn(&mut EditorClient, &mut Vec<MapResourceRefSkeleton<U>>, &Path, Vec<u8>),
    R: Fn(&mut EditorClient, &mut Vec<MapResourceRefSkeleton<U>>, usize),
{
    ui.vertical(|ui| {
        let mut del_index = None;
        for (index, resource) in resources.iter().enumerate() {
            ui.with_layout(Layout::right_to_left(egui::Align::Min), |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                if ui.button(icon_font_text(ui, "\u{f2ed}")).clicked() {
                    del_index = Some(index);
                }

                ui.vertical_centered_justified(|ui| {
                    if ui.add(Button::new(&resource.def.name)).clicked() {
                        // show resource?
                    }
                });
            });
        }

        if let Some(index) = del_index {
            rem_resource(client, resources, index);
        }

        if ui.button(icon_font_text(ui, "\u{f0fe}")).clicked() {
            panel_data.file_dialog.select_file();
        }
    });

    panel_data.loading_tasks = panel_data
        .loading_tasks
        .drain()
        .filter_map(|(name, task)| {
            if task.is_finished() {
                if let Ok(file) = task.get_storage() {
                    load_resource(client, resources, name.as_ref(), file);
                }
                None
            } else {
                Some((name, task))
            }
        })
        .collect();

    let file_dialog = &mut panel_data.file_dialog;
    if !main_frame_only && file_dialog.state() == DialogState::Open {
        let mode = file_dialog.mode();
        if let Some(selected) = file_dialog.update(ui.ctx()).selected() {
            match mode {
                DialogMode::SelectFile => {
                    let selected = selected.to_path_buf();
                    let fs = io.fs.clone();
                    panel_data.loading_tasks.insert(
                        selected.to_path_buf(),
                        io.io_batcher
                            .spawn(async move { read_file_editor(&fs, selected.as_ref()).await }),
                    );
                }
                DialogMode::SelectDirectory | DialogMode::SaveFile => {
                    panic!("")
                }
                DialogMode::SelectMultiple => {
                    panic!("multi select currently isn't implemented.")
                }
            }
        }
    }
}
