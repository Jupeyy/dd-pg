use egui::{text::LayoutJob, Color32};
use egui_extras::Size;
use math::math::vector::vec2;
use ui_base::types::{UiRenderPipe, UiState};

use crate::{ui::user_data::UserDataWithTab, utils::ui_pos_to_world_pos};

pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserDataWithTab>,
    ui_state: &mut UiState,
    main_frame_only: bool,
) {
    let editor_tab = &mut *pipe.user_data.editor_tab;
    let style = ui.style();
    let item_height = style.spacing.interact_size.y;
    let row_height = item_height + style.spacing.item_spacing.y;
    let height = row_height * 2.0;
    egui::TopBottomPanel::bottom("bottom_panel")
        .resizable(false)
        .default_height(height)
        .height_range(height..=height)
        .show_inside(ui, |ui| {
            egui::ScrollArea::horizontal().show(ui, |ui| {
                if main_frame_only {
                } else {
                    ui.vertical(|ui| {
                        egui_extras::StripBuilder::new(ui)
                            .size(Size::exact(item_height))
                            .size(Size::exact(row_height))
                            .clip(true)
                            .vertical(|mut strip| {
                                strip.cell(|ui| {
                                    ui.horizontal(|ui| {
                                        if ui
                                            .add(egui::Button::new("Animations").selected(
                                                editor_tab.map.user.ui_values.animations_panel_open,
                                            ))
                                            .clicked()
                                        {
                                            editor_tab.map.user.ui_values.animations_panel_open =
                                                !editor_tab
                                                    .map
                                                    .user
                                                    .ui_values
                                                    .animations_panel_open;
                                        }
                                        ui.button("Server settings");
                                    });
                                });
                                strip.cell(|ui| {
                                    egui_extras::StripBuilder::new(ui)
                                        .size(Size::exact(180.0))
                                        .size(Size::exact(100.0))
                                        .size(Size::remainder())
                                        .clip(true)
                                        .horizontal(|mut strip| {
                                            strip.cell(|ui| {
                                                let mut layout = LayoutJob::default();
                                                let number_format = egui::TextFormat {
                                                    color: Color32::from_rgb(100, 100, 255),
                                                    ..Default::default()
                                                };
                                                layout.append(
                                                    "camera (",
                                                    0.0,
                                                    egui::TextFormat::default(),
                                                );
                                                layout.append(
                                                    &format!(
                                                        "{:.2}",
                                                        editor_tab.map.groups.user.pos.x,
                                                    ),
                                                    0.0,
                                                    number_format.clone(),
                                                );
                                                layout.append(
                                                    ", ",
                                                    0.0,
                                                    egui::TextFormat::default(),
                                                );
                                                layout.append(
                                                    &format!(
                                                        "{:.2}",
                                                        editor_tab.map.groups.user.pos.y
                                                    ),
                                                    0.0,
                                                    number_format.clone(),
                                                );
                                                layout.append(
                                                    ")",
                                                    0.0,
                                                    egui::TextFormat::default(),
                                                );
                                                ui.label(layout);
                                            });
                                            strip.cell(|ui| {
                                                let mut layout = LayoutJob::default();
                                                let number_format = egui::TextFormat {
                                                    color: Color32::from_rgb(100, 100, 255),
                                                    ..Default::default()
                                                };
                                                layout.append(
                                                    " zoom (",
                                                    0.0,
                                                    egui::TextFormat::default(),
                                                );
                                                layout.append(
                                                    &format!(
                                                        "{:.2}",
                                                        editor_tab.map.groups.user.zoom
                                                    ),
                                                    0.0,
                                                    number_format.clone(),
                                                );
                                                layout.append(
                                                    ")",
                                                    0.0,
                                                    egui::TextFormat::default(),
                                                );
                                                ui.label(layout);
                                            });
                                            strip.cell(|ui| {
                                                if let Some(cursor_pos) =
                                                    ui.input(|i| i.pointer.latest_pos())
                                                {
                                                    let mut layout = LayoutJob::default();
                                                    let number_format = egui::TextFormat {
                                                        color: Color32::from_rgb(100, 100, 255),
                                                        ..Default::default()
                                                    };
                                                    let pos = ui_pos_to_world_pos(
                                                        pipe.user_data.canvas_handle,
                                                        editor_tab.map.groups.user.zoom,
                                                        vec2::new(cursor_pos.x, cursor_pos.y),
                                                        editor_tab.map.groups.user.pos.x,
                                                        editor_tab.map.groups.user.pos.y,
                                                        0.0,
                                                        0.0,
                                                        100.0,
                                                        100.0,
                                                    );
                                                    layout.append(
                                                        " mouse (",
                                                        0.0,
                                                        egui::TextFormat::default(),
                                                    );
                                                    layout.append(
                                                        &format!("{:.2}", pos.x),
                                                        0.0,
                                                        number_format.clone(),
                                                    );
                                                    layout.append(
                                                        ", ",
                                                        0.0,
                                                        egui::TextFormat::default(),
                                                    );
                                                    layout.append(
                                                        &format!("{:.2}", pos.y),
                                                        0.0,
                                                        number_format.clone(),
                                                    );
                                                    layout.append(
                                                        ")",
                                                        0.0,
                                                        egui::TextFormat::default(),
                                                    );
                                                    ui.label(layout);
                                                }
                                            });
                                        });
                                });
                            });
                    });
                }
            });
        });
}
