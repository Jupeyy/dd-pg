use std::net::SocketAddr;

use egui::{epaint::RectShape, Color32, Frame, Shape};
use egui_extras::{Size, StripBuilder};

use ui_base::types::{UiRenderPipe, UiState};

use crate::main_menu::user_data::UserData;

use super::info_panel::player_list::list::entry::EntryData;

/// big box, rounded edges
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    cur_page: &str,
    main_frame_only: bool,
) {
    let w = ui.available_width();
    let margin = ui.style().spacing.item_spacing.x;
    let width_details = 300.0;
    let width_browser = w - width_details - margin;
    StripBuilder::new(ui)
        .size(Size::exact(width_browser))
        .size(Size::remainder())
        .horizontal(|mut strip| {
            strip.cell(|ui| {
                if main_frame_only {
                    ui.painter().add(Shape::Rect(RectShape::filled(
                        ui.available_rect_before_wrap(),
                        5.0,
                        Color32::from_rgba_unmultiplied(0, 0, 0, 255),
                    )));
                } else {
                    Frame::default()
                        .fill(Color32::from_rgba_unmultiplied(0, 0, 0, 100))
                        .rounding(5.0)
                        .show(ui, |ui| {
                            let filter_height = 30.0;
                            let bottom_bar_height = 30.0;
                            StripBuilder::new(ui)
                                .size(Size::exact(0.0))
                                .size(Size::exact(filter_height))
                                .size(Size::remainder())
                                .size(Size::exact(bottom_bar_height))
                                .size(Size::exact(0.0))
                                .clip(true)
                                .vertical(|mut strip| {
                                    strip.empty();
                                    strip.cell(|ui| {
                                        super::filter::render(ui, pipe, ui_state);
                                    });
                                    strip.cell(|ui| {
                                        super::list::list::render(ui, pipe, cur_page);
                                    });
                                    strip.cell(|ui| {
                                        super::bottom_bar::render(ui, pipe);
                                    });
                                    strip.empty();
                                });
                        });
                }
            });
            strip.cell(|ui| {
                StripBuilder::new(ui)
                    .size(Size::remainder())
                    .size(Size::exact(0.0))
                    .size(Size::remainder())
                    .vertical(|mut strip| {
                        strip.cell(|ui| {
                            let browser_data = &pipe.user_data.browser_data;
                            let server = browser_data.servers.iter().find(|server| {
                                server.address
                                    == pipe
                                        .user_data
                                        .config
                                        .storage_opt::<SocketAddr>("server-addr")
                                        .map(|a| a.to_string())
                                        .unwrap_or_default()
                            });
                            super::info_panel::main_frame::render(
                                ui,
                                &ui.ctx().screen_rect().clone(),
                                &mut UiRenderPipe {
                                    cur_time: pipe.cur_time,
                                    user_data: &mut EntryData {
                                        stream_handle: pipe.user_data.stream_handle,
                                        canvas_handle: pipe.user_data.canvas_handle,
                                        skin_container: pipe.user_data.skin_container,
                                        render_tee: pipe.user_data.render_tee,
                                        flags_container: pipe.user_data.flags_container,
                                        full_rect: pipe.user_data.full_rect,
                                    },
                                },
                                ui_state,
                                main_frame_only,
                                server,
                            );
                        });
                        strip.empty();
                        strip.cell(|ui| {
                            super::friend_list::main_frame::render(
                                ui,
                                pipe,
                                ui_state,
                                main_frame_only,
                            );
                        });
                    });
            });
        });
}
