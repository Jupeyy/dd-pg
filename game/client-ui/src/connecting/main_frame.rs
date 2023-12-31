use egui::{Pos2, Rect, Vec2, Color32};
use graphics::graphics::Graphics;
use ui_base::types::{UIPipe, UIState};

use super::user_data::{ConnectMode, UserData};

/// top bar
/// big square, rounded edges
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    _ui_state: &mut UIState,
    _graphics: &mut Graphics,
    main_frame_only: bool,
) {
    let width = ui.available_width().min(150.0);
    let height = ui.available_height().min(50.0);
    let offset_x = (ui.available_width() / 2.0) - (width / 2.0);
    let offset_y = (ui.available_height() / 2.0) - (height / 2.0);
    ui.allocate_ui_at_rect(
        Rect::from_min_size(Pos2::new(offset_x, offset_y), Vec2::new(width, height)),
        |ui| {
            if main_frame_only {
                ui.painter().rect_filled(
                    ui.available_rect_before_wrap(),
                    5.0,
                    Color32::from_rgba_unmultiplied(0,0,0,255),
                );
            } else {
                match pipe.user_data.mode {
                    ConnectMode::Connecting => {
                        ui.vertical(|ui| {
                            ui.label(&format!(
                                "connecting to:\n{}",
                                pipe.config.ui.last_server_addr
                            ));
                            if ui.button("cancel").clicked() {
                                pipe.ui_feedback.network_disconnect();
                                pipe.ui_feedback.call_path(pipe.config, "", "");
                            }
                        });
                    }
                    ConnectMode::Err { msg } => {
                        ui.vertical(|ui| {
                            ui.label(&format!(
                                "connecting to {} failed:\n{}",
                                pipe.config.ui.last_server_addr, msg
                            ));
                            if ui.button("return").clicked() {
                                pipe.ui_feedback.network_disconnect();
                                pipe.ui_feedback.call_path(pipe.config, "", "");
                            }
                        });
                    }
                    ConnectMode::Queue { msg } => {
                        ui.vertical(|ui| {
                            ui.label(&format!(
                                "connecting to {}",
                                pipe.config.ui.last_server_addr
                            ));
                            ui.label(&format!("waiting in queue: {}", msg));
                            if ui.button("cancel").clicked() {
                                pipe.ui_feedback.network_disconnect();
                                pipe.ui_feedback.call_path(pipe.config, "", "");
                            }
                        });
                    }
                }
            }
        },
    );
}
