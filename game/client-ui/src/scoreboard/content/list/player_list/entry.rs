use egui::{Layout, Rect, RichText};
use egui_extras::{Size, StripBuilder};
use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use math::math::vector::vec2;
use ui_base::types::{UIPipe, UIState};

use crate::scoreboard::{
    content::list::definitions::TABLE_COLUMNS, user_data::UserData,
    utils::render_tee_for_scoreboard,
};

/// single player entry
pub fn render<B: GraphicsBackendInterface>(
    ui: &mut egui::Ui,
    _pipe: &mut UIPipe<UserData>,
    _ui_state: &mut UIState,
    _graphics: &mut GraphicsBase<B>,
    full_ui_rect: &Rect,
    player_index: usize,
) {
    const FONT_SIZE: f32 = 9.0;

    ui.set_clip_rect(ui.available_rect_before_wrap());

    let mut width_left = ui.available_width();

    let mut strip = StripBuilder::new(ui);
    let mut col_count = 0;
    while width_left > 0.0 {
        if col_count < TABLE_COLUMNS.len() {
            let col_width = TABLE_COLUMNS[col_count];
            width_left -= col_width;
            strip = strip.size(Size::exact(col_width));
            col_count += 1;
        } else {
            break;
        }
    }
    strip = strip.clip(true);
    strip.horizontal(|mut strip| {
        for i in 0..col_count {
            match i {
                0 => {
                    strip.cell(|ui| {
                        ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.label(RichText::new(player_index.to_string()).size(FONT_SIZE));
                        });
                    });
                }
                1 => {
                    strip.cell(|ui| {
                        let this_rect = ui.available_rect_before_wrap();
                        render_tee_for_scoreboard::<B>(
                            ui,
                            *full_ui_rect,
                            vec2::new(
                                this_rect.min.x + this_rect.width() / 2.0,
                                this_rect.min.y + this_rect.height() / 2.0,
                            ),
                            15.0,
                        );
                    });
                }
                2 => {
                    strip.cell(|ui| {
                        ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.label(RichText::new("WWWWWWWWWWWWWWW").size(FONT_SIZE));
                        });
                    });
                }
                3 => {
                    strip.cell(|ui| {
                        ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.label(RichText::new("MMMMMMMMMM").size(FONT_SIZE));
                        });
                    });
                }
                4 => {
                    strip.cell(|ui| {});
                }
                _ => {
                    strip.cell(|ui| {
                        ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.label(RichText::new("999").size(FONT_SIZE));
                        });
                    });
                }
            }
        }
    });
}
