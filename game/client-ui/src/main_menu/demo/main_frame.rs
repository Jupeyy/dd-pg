use std::path::PathBuf;

use base::duration_ext::DurationToRaceStr;
use egui::{Align2, Color32, ComboBox, DragValue, Grid, Vec2};
use egui_extras::{Size, StripBuilder};
use ui_base::{
    types::UiRenderPipe,
    utils::{add_horizontal_margins, icon_font_plus_text},
};

use crate::{
    events::UiEvent,
    main_menu::{constants::MENU_DEMO_NAME, user_data::UserData},
};

fn record_settings(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>) {
    egui::Window::new("Export demo to video")
        .anchor(Align2::CENTER_CENTER, Vec2::default())
        .show(ui.ctx(), |ui| {
            let config = &mut pipe.user_data.config;
            let file_name = config
                .engine
                .ui
                .path
                .query
                .entry("video-file-name".to_string())
                .or_default();
            Grid::new("record-settings").num_columns(2).show(ui, |ui| {
                let config = &mut config.game;
                ui.label("Name:");
                ui.text_edit_singleline(file_name);
                ui.end_row();

                ui.label("Fps:");
                ui.add(DragValue::new(&mut config.cl.recorder.fps));
                ui.end_row();

                ui.label("Width:");
                ui.add(DragValue::new(&mut config.cl.recorder.width));
                ui.end_row();

                ui.label("Height:");
                ui.add(DragValue::new(&mut config.cl.recorder.height));
                ui.end_row();

                ui.label("Pixels per point (similar to DPI):");
                ui.add(DragValue::new(&mut config.cl.recorder.pixels_per_point));
                ui.end_row();

                ui.label("Crf (0 = lossless, 51 = worst):");
                ui.add(DragValue::new(&mut config.cl.recorder.crf).range(0..=51));
                ui.end_row();

                ui.label("Hardware acceleration (GPU):");
                ComboBox::new("hw_accel_combobox", "")
                    .selected_text(&config.cl.recorder.hw_accel)
                    .show_ui(ui, |ui| {
                        if ui.button("None").clicked() {
                            config.cl.recorder.hw_accel = "".to_string();
                        }
                        if ui.button("VAAPI (Linux)").clicked() {
                            config.cl.recorder.hw_accel = "vaapi".to_string();
                        }
                        if ui.button("Cuda (NVIDIA)").clicked() {
                            config.cl.recorder.hw_accel = "cuda".to_string();
                        }
                        if ui.button("AMF (AMD on Windows)").clicked() {
                            config.cl.recorder.hw_accel = "amf".to_string();
                        }
                    });
                ui.end_row();
            });
            let video_name = file_name.clone();
            ui.horizontal(|ui| {
                if ui.button("Abort").clicked() {
                    config.path().query.remove("recorder-clicked");
                }
                if ui.button("Ok").clicked() {
                    let cur_path: String = config.storage("demo-path");
                    let cur_path: PathBuf = cur_path.into();
                    let name: String = config.storage("selected-demo");

                    let demo_path = cur_path.join(name);

                    pipe.user_data.events.push(UiEvent::EncodeDemoToVideo {
                        name: demo_path,
                        video_name,
                    });
                    config.path().query.remove("recorder-clicked");
                }
            });
        });
}

pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    cur_page: &str,
    main_frame_only: bool,
) {
    if cur_page == MENU_DEMO_NAME {
        StripBuilder::new(ui)
            .size(Size::remainder())
            .size(Size::exact(300.0))
            .horizontal(|mut strip| {
                strip.cell(|ui| {
                    egui::Frame::none()
                        .fill(Color32::from_rgba_unmultiplied(0, 0, 0, 100))
                        .rounding(5.0)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.set_height(ui.available_height());
                            if !main_frame_only {
                                add_horizontal_margins(ui, |ui| {
                                    StripBuilder::new(ui)
                                        .size(Size::exact(30.0))
                                        .size(Size::remainder())
                                        .size(Size::exact(30.0))
                                        .vertical(|mut strip| {
                                            strip.cell(|ui| {
                                                super::search::render(ui, pipe);
                                            });
                                            strip.cell(|ui| {
                                                super::list::render(ui, pipe);
                                            });
                                            strip.cell(|ui| {
                                                ui.horizontal(|ui| {
                                                    if ui.button("play").clicked() {
                                                        let cur_path: String = pipe
                                                            .user_data
                                                            .config
                                                            .storage("demo-path");
                                                        let cur_path: PathBuf = cur_path.into();
                                                        let name: String = pipe
                                                            .user_data
                                                            .config
                                                            .storage("selected-demo");

                                                        let new_path = cur_path.join(name);
                                                        pipe.user_data.events.push(
                                                            UiEvent::PlayDemo { name: new_path },
                                                        );
                                                    }
                                                    if ui.button("record").clicked() {
                                                        pipe.user_data.config.path().query.insert(
                                                            "recorder-clicked".to_string(),
                                                            "true".to_string(),
                                                        );
                                                    }

                                                    if pipe
                                                        .user_data
                                                        .config
                                                        .path()
                                                        .query
                                                        .contains_key("recorder-clicked")
                                                    {
                                                        let name: String = pipe
                                                            .user_data
                                                            .config
                                                            .storage("selected-demo");
                                                        let file_name = pipe
                                                            .user_data
                                                            .config
                                                            .path()
                                                            .query
                                                            .entry("video-file-name".to_string())
                                                            .or_default();
                                                        let name: PathBuf = name.into();
                                                        *file_name = name
                                                            .file_stem()
                                                            .map(|s| {
                                                                s.to_string_lossy().to_string()
                                                            })
                                                            .unwrap_or_default();
                                                        record_settings(ui, pipe);
                                                    }
                                                });
                                            });
                                        });
                                });
                            }
                        });
                });
                strip.cell(|ui| {
                    egui::Frame::none()
                        .fill(Color32::from_rgba_unmultiplied(0, 0, 0, 100))
                        .rounding(5.0)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.set_height(ui.available_height());
                            if !main_frame_only {
                                StripBuilder::new(ui)
                                    .size(Size::exact(30.0))
                                    .size(Size::remainder())
                                    .vertical(|mut strip| {
                                        strip.cell(|ui| {
                                            ui.centered_and_justified(|ui| {
                                                ui.label(icon_font_plus_text(
                                                    ui,
                                                    "\u{f05a}",
                                                    "Demo information",
                                                ));
                                            });
                                        });
                                        strip.cell(|ui| {
                                            if let Some((header, header_ext)) =
                                                pipe.user_data.demo_info
                                            {
                                                Grid::new("demo-info").num_columns(2).show(
                                                    ui,
                                                    |ui| {
                                                        ui.label("Map:");
                                                        ui.label(header_ext.map.as_str());
                                                        ui.end_row();
                                                        ui.label("Length:");
                                                        ui.label(header.len.to_race_string());
                                                        ui.end_row();
                                                    },
                                                );
                                            }
                                        });
                                    });
                            }
                        });
                });
            });
    }
}
