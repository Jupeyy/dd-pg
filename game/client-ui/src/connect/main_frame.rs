use std::net::SocketAddr;

use egui::{Color32, Pos2, Rect, UiBuilder, Vec2};

use ui_base::types::UiRenderPipe;

use crate::events::UiEvent;

use super::user_data::{ConnectModes, UserData};

/// top bar
/// big square, rounded edges
pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>, main_frame_only: bool) {
    let width = ui.available_width().min(150.0);
    let height = ui.available_height().min(50.0);
    let offset_x = (ui.available_width() / 2.0) - (width / 2.0);
    let offset_y = (ui.available_height() / 2.0) - (height / 2.0);
    ui.allocate_new_ui(
        UiBuilder::new().max_rect(Rect::from_min_size(
            Pos2::new(offset_x, offset_y),
            Vec2::new(width, height),
        )),
        |ui| {
            if main_frame_only {
                ui.painter().rect_filled(
                    ui.available_rect_before_wrap(),
                    5.0,
                    Color32::from_rgba_unmultiplied(0, 0, 0, 255),
                );
            } else {
                match pipe.user_data.mode.get() {
                    ConnectModes::Connecting => {
                        ui.vertical(|ui| {
                            ui.label(format!(
                                "connecting to:\n{}",
                                pipe.user_data
                                    .config
                                    .storage_opt::<SocketAddr>("server-addr")
                                    .map(|a| a.to_string())
                                    .unwrap_or_default()
                            ));
                            if ui.button("cancel").clicked() {
                                pipe.user_data.events.push(UiEvent::Disconnect);
                                pipe.user_data.config.engine.ui.path.route("");
                            }
                        });
                    }
                    ConnectModes::ConnectingErr { msg } => {
                        ui.vertical(|ui| {
                            ui.label(format!(
                                "connecting to {} failed:\n{}",
                                pipe.user_data
                                    .config
                                    .storage_opt::<SocketAddr>("server-addr")
                                    .map(|a| a.to_string())
                                    .unwrap_or_default(),
                                msg
                            ));
                            if ui.button("return").clicked() {
                                pipe.user_data.events.push(UiEvent::Disconnect);
                                pipe.user_data.config.engine.ui.path.route("");
                            }
                        });
                    }
                    ConnectModes::Queue { msg } => {
                        ui.vertical(|ui| {
                            ui.label(format!(
                                "connecting to {}",
                                pipe.user_data
                                    .config
                                    .storage_opt::<SocketAddr>("server-addr")
                                    .map(|a| a.to_string())
                                    .unwrap_or_default()
                            ));
                            ui.label(format!("waiting in queue: {}", msg));
                            if ui.button("cancel").clicked() {
                                pipe.user_data.events.push(UiEvent::Disconnect);
                                pipe.user_data.config.engine.ui.path.route("");
                            }
                        });
                    }
                    ConnectModes::DisconnectErr { msg } => {
                        ui.vertical(|ui| {
                            ui.label(format!(
                                "connection to {} lost:\n{}",
                                pipe.user_data
                                    .config
                                    .storage_opt::<SocketAddr>("server-addr")
                                    .map(|a| a.to_string())
                                    .unwrap_or_default(),
                                msg
                            ));
                            if ui.button("return").clicked() {
                                pipe.user_data.events.push(UiEvent::Disconnect);
                                pipe.user_data.config.engine.ui.path.route("");
                            }
                        });
                    }
                }
            }
        },
    );
}
