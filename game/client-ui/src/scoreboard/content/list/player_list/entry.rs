use egui::{Layout, Rect, RichText};
use egui_extras::{Size, StripBuilder};
use graphics::graphics::Graphics;
use math::math::vector::vec2;
use shared_game::types::types::PlayerScoreboardInfo;
use ui_base::types::{UIPipe, UIState};

use crate::scoreboard::{
    content::list::definitions::{
        TABLE_CONTENT_COLUMN_SPACING, TABLE_CONTENT_FONT_SIZES, TABLE_CONTENT_TEE_SIZES,
        TABLE_CONTENT_WIDTH, TABLE_NAME_COLUMN_INDEX,
    },
    user_data::UserData,
    utils::render_tee_for_scoreboard,
};

/// single player entry
pub fn render(
    ui: &mut egui::Ui,
    _pipe: &mut UIPipe<UserData>,
    _ui_state: &mut UIState,
    _graphics: &mut Graphics,
    players: &[PlayerScoreboardInfo],
    full_ui_rect: &Rect,
    player_index: usize,
    font_size_index: usize,
) {
    let font_size = TABLE_CONTENT_FONT_SIZES[font_size_index];
    let tee_size = TABLE_CONTENT_TEE_SIZES[font_size_index];

    ui.set_clip_rect(ui.available_rect_before_wrap());

    let mut width_left = ui.available_width();
    let spacing_x = TABLE_CONTENT_COLUMN_SPACING[font_size_index];
    ui.style_mut().spacing.item_spacing.x = spacing_x;

    let mut strip = StripBuilder::new(ui);
    let mut col_count = 0;
    while width_left > 0.0 {
        if col_count < TABLE_CONTENT_WIDTH[font_size_index].len() {
            let col_width = TABLE_CONTENT_WIDTH[font_size_index][col_count];
            if width_left >= col_width {
                width_left -= col_width + spacing_x;
                if col_count == TABLE_NAME_COLUMN_INDEX {
                    strip = strip.size(Size::remainder());
                } else {
                    strip = strip.size(Size::exact(col_width));
                }
                col_count += 1;
            } else {
                break;
            }
        } else {
            break;
        }
    }
    let player = &players[player_index];
    strip = strip.clip(true);
    strip.horizontal(|mut strip| {
        for i in 0..col_count {
            match i {
                0 => {
                    strip.cell(|ui| {
                        ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.label(RichText::new(player.score.to_string()).size(font_size));
                        });
                    });
                }
                1 => {
                    strip.cell(|ui| {
                        let this_rect = ui.available_rect_before_wrap();
                        render_tee_for_scoreboard(
                            ui,
                            *full_ui_rect,
                            Some(ui.clip_rect()),
                            &player.skin_name,
                            vec2::new(
                                this_rect.min.x + this_rect.width() / 2.0,
                                this_rect.min.y + this_rect.height() / 2.0,
                            ),
                            tee_size,
                        );
                    });
                }
                2 => {
                    strip.cell(|ui| {
                        ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.label(RichText::new(player.player_name.as_str()).size(font_size));
                        });
                    });
                }
                3 => {
                    strip.cell(|ui| {
                        ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.label(RichText::new(player.clan_name.as_str()).size(font_size));
                        });
                    });
                }
                4 => {
                    strip.cell(|ui| {});
                }
                _ => {
                    strip.cell(|ui| {
                        ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.label(RichText::new(player.ping.to_string()).size(font_size));
                        });
                    });
                }
            }
        }
    });
}
