use egui::{Layout, ScrollArea};
use ui_base::types::{UiRenderPipe, UiState};

use crate::{events::UiEvent, main_menu::user_data::UserData};

pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>, ui_state: &mut UiState) {
    ui.with_layout(Layout::top_down(egui::Align::Min), |ui| {
        let wnd = &mut pipe.user_data.config.engine.wnd;
        let mut changed = false;

        egui::ComboBox::new("fullscreen_mode", "Fullscreen mode")
            .selected_text(if wnd.fullscreen {
                "fullscreen"
            } else if !wnd.fullscreen && wnd.maximized && !wnd.decorated {
                "borderless-fullscreen"
            } else {
                "windowed"
            })
            .show_ui(ui, |ui| {
                ui.vertical(|ui| {
                    if ui.add(egui::Button::new("fullscreen")).clicked() {
                        wnd.fullscreen = true;
                        changed |= true;
                    }
                    if ui.add(egui::Button::new("borderless-fullscreen")).clicked() {
                        wnd.fullscreen = false;
                        wnd.decorated = false;
                        wnd.maximized = true;
                        changed |= true;
                    }
                    if ui.add(egui::Button::new("windowed")).clicked() {
                        wnd.fullscreen = false;
                        wnd.decorated = true;
                        changed |= true;
                    }
                })
            });

        if let Some(monitors) = pipe.user_data.monitors.monitors().iter().next() {
            ui.label(&monitors.name);
            ScrollArea::vertical().show(ui, |ui| {
                for mode in &monitors.video_modes {
                    if ui
                        .button(format!(
                            "{}x{} @{:0.2}",
                            mode.width,
                            mode.height,
                            mode.refresh_rate_mhz as f64 / 1000.0
                        ))
                        .clicked()
                    {
                        changed |= true;
                        wnd.width = mode.width;
                        wnd.height = mode.height;
                        wnd.refresh_rate_mhz = mode.refresh_rate_mhz;
                    }
                }
            });
        }

        if changed {
            pipe.user_data.events.push(UiEvent::WindowChange);
        }
    });
}
