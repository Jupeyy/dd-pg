use client_containers_new::container::ContainerItemIndexType;
use egui::{Color32, FontId, Frame, Label, Layout, RichText, Sense};
use game_interface::types::resource_key::NetworkResourceKey;
use math::math::vector::vec2;
use ui_base::types::{UiRenderPipe, UiState};

use crate::{main_menu::user_data::UserData, utils::render_tee_for_ui};

/// single server list entry
pub fn render(
    ui: &mut egui::Ui,
    skin: &str,
    ty: ContainerItemIndexType,
    player_index: usize,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
) {
    let skin_valid: Result<NetworkResourceKey<24>, _> = skin.try_into();

    let skin_size = 100.0;
    let entry_size = skin_size + 25.0;
    let player = &mut pipe.user_data.config.game.players[player_index];
    let (rect, sense) = ui.allocate_exact_size(egui::vec2(entry_size, entry_size), Sense::click());
    ui.allocate_ui_at_rect(rect, |ui| {
        ui.with_layout(
            Layout::top_down(egui::Align::Center)
                .with_main_justify(true)
                .with_cross_justify(true)
                .with_main_wrap(true),
            |ui| {
                let mut clicked = sense.clicked();
                Frame::default()
                    .fill(if player.skin.name == skin {
                        Color32::from_rgba_unmultiplied(0, 0, 50, 100)
                    } else {
                        Color32::from_rgba_unmultiplied(0, 0, 0, 100)
                    })
                    .rounding(5.0)
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            let skin_rect = ui.available_rect_before_wrap();
                            if matches!(ty, ContainerItemIndexType::Http) {
                                ui.painter().text(
                                    skin_rect.min + egui::vec2(5.0, 5.0),
                                    egui::Align2::LEFT_TOP,
                                    "\u{f019}",
                                    FontId::new(10.0, egui::FontFamily::Name("icons".into())),
                                    Color32::WHITE,
                                );
                            }

                            let pos = vec2::new(
                                skin_rect.min.x + entry_size / 2.0,
                                skin_rect.min.y + skin_size / 2.0,
                            );

                            if let Err(skin_valid) = &skin_valid {
                                ui.label(RichText::new(skin_valid.to_string()).color(Color32::RED));
                            }

                            let rect = ui.available_rect_before_wrap();
                            let height_diff = skin_rect.height() - rect.height();
                            let _ =
                                ui.allocate_space(egui::vec2(entry_size, skin_size - height_diff));
                            clicked |= ui
                                .with_layout(
                                    Layout::top_down(egui::Align::Center).with_cross_justify(true),
                                    |ui| ui.add(Label::new(format!("{}", skin)).wrap(true)),
                                )
                                .inner
                                .clicked();
                            ui.add_space(ui.available_height());

                            if skin_valid.is_ok()
                                && ui.is_rect_visible(egui::Rect::from_min_size(
                                    skin_rect.min,
                                    egui::vec2(entry_size, entry_size),
                                ))
                            {
                                render_tee_for_ui(
                                    pipe.user_data.canvas_handle,
                                    pipe.user_data.skin_container,
                                    pipe.user_data.render_tee,
                                    ui,
                                    ui_state,
                                    pipe.user_data.full_rect,
                                    Some(ui.clip_rect()),
                                    &skin.try_into().unwrap_or_default(),
                                    pos,
                                    skin_size,
                                );
                            }
                        });
                    });
                if clicked {
                    player.skin.name = skin.to_string();
                }
            },
        );
    });
}
