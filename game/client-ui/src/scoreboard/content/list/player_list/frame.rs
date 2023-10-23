use std::ops::Range;

use egui::Rect;
use egui_extras::{Size, StripBuilder};
use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use ui_base::types::{UIPipe, UIState};

use crate::scoreboard::user_data::UserData;

/// player list frame
pub fn render<B: GraphicsBackendInterface>(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    ui_state: &mut UIState,
    graphics: &mut GraphicsBase<B>,
    full_ui_rect: &Rect,
    index_range: Range<usize>,
    item_height: f32,
) {
    let mut strip = StripBuilder::new(ui);
    for _ in index_range.clone() {
        strip = strip.size(Size::exact(item_height));
    }
    strip.vertical(|mut strip| {
        for i in index_range {
            strip.cell(|ui| {
                super::entry::render(ui, pipe, ui_state, graphics, full_ui_rect, i);
            });
        }
    });
}
