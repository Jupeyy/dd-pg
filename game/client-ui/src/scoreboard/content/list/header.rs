use egui::{Layout, RichText};
use egui_extras::{Size, StripBuilder};

use ui_base::types::{UIPipe, UIState};

use crate::scoreboard::{
    content::list::definitions::{
        TABLE_CONTENT_COLUMN_SPACING, TABLE_CONTENT_WIDTH, TABLE_NAME_COLUMN_INDEX,
    },
    user_data::UserData,
};

/// table header
pub fn render(
    ui: &mut egui::Ui,
    _pipe: &mut UIPipe<UserData>,
    _ui_state: &mut UIState,
    font_size_index: usize,
) {
    const FONT_SIZE: f32 = 8.0;

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
    strip = strip.clip(true);
    strip.horizontal(|mut strip| {
        for i in 0..col_count {
            match i {
                0 => {
                    strip.cell(|ui| {
                        ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.label(RichText::new("points").size(FONT_SIZE));
                        });
                    });
                }
                1 => {
                    strip.empty();
                }
                2 => {
                    strip.cell(|ui| {
                        ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.label(RichText::new("name").size(FONT_SIZE));
                        });
                    });
                }
                3 => {
                    strip.cell(|ui| {
                        ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.label(RichText::new("clan").size(FONT_SIZE));
                        });
                    });
                }
                4 => {
                    strip.empty();
                }
                _ => {
                    strip.cell(|ui| {
                        ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.label(RichText::new("ping").size(FONT_SIZE));
                        });
                    });
                }
            }
        }
    });
}
