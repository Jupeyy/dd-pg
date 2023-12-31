use std::ops::Range;

use egui::Rect;
use egui_extras::{Size, StripBuilder};
use graphics::graphics::Graphics;
use shared_game::types::types::PlayerScoreboardInfo;
use ui_base::types::{UIPipe, UIState};

use crate::scoreboard::{
    content::list::definitions::TABLE_CONTENT_ROW_HEIGHTS, user_data::UserData,
};

/// player list frame
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    ui_state: &mut UIState,
    graphics: &mut Graphics,
    players: &[PlayerScoreboardInfo],
    full_ui_rect: &Rect,
    index_range: Range<usize>,
    font_size_index: usize,
) {
    let item_height = TABLE_CONTENT_ROW_HEIGHTS[font_size_index];
    let mut strip = StripBuilder::new(ui);
    for _ in index_range.clone() {
        strip = strip.size(Size::exact(item_height));
    }
    strip.vertical(|mut strip| {
        for i in index_range {
            strip.cell(|ui| {
                super::entry::render(
                    ui,
                    pipe,
                    ui_state,
                    graphics,
                    players,
                    full_ui_rect,
                    i,
                    font_size_index,
                );
            });
        }
    });
}
