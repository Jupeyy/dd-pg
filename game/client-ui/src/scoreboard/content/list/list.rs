use egui::{epaint::RectShape, Color32, Rect};
use egui_extras::{Size, StripBuilder};
use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use ui_base::types::{UIPipe, UIState};

use crate::scoreboard::user_data::UserData;

/// table header + player list
pub fn render<B: GraphicsBackendInterface>(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    ui_state: &mut UIState,
    graphics: &mut GraphicsBase<B>,
    full_ui_rect: &Rect,
) {
    let player_count = 64;
    let height_header = 12.0;
    let spacing_x = ui.style().spacing.item_spacing.x;
    let spacing_y = ui.style().spacing.item_spacing.y;
    let height_of_list = ui.available_height() - (4.0 * spacing_y + height_header);

    let height_of_item = 18.0;
    let items_per_height = ((height_of_list / (height_of_item + spacing_y)) as usize).max(1);
    let columns = ((player_count.max(1) - 1) / items_per_height) + 1;

    let total_width = ui.available_width();
    let width_per_column =
        (total_width / columns as f32 - spacing_x * (columns - 1) as f32).max(150.0);

    StripBuilder::new(ui)
        .size(Size::exact(0.0))
        .size(Size::exact(height_header))
        .size(Size::exact(0.0))
        .size(Size::remainder())
        .size(Size::exact(0.0))
        .vertical(|mut strip| {
            strip.empty();
            strip.cell(|ui| {
                ui.painter().add(RectShape::filled(
                    ui.available_rect_before_wrap(),
                    0.0,
                    Color32::DARK_GRAY,
                ));
                let mut strip = StripBuilder::new(ui);
                let mut remaining_width = total_width;
                let mut col_count = 0;
                for i in 0..columns {
                    remaining_width -= if i > 0 { spacing_x } else { 0.0 };
                    strip = strip.size(Size::exact(width_per_column.min(remaining_width)));
                    remaining_width -= width_per_column;
                    col_count += 1;
                    if remaining_width <= 0.0 {
                        break;
                    }
                }
                strip = strip.clip(true);
                strip.horizontal(|mut strip| {
                    for _ in 0..col_count {
                        strip.cell(|ui| {
                            ui.set_clip_rect(ui.available_rect_before_wrap());
                            super::header::render(ui, pipe, ui_state, graphics);
                        });
                    }
                });
            });
            strip.empty();
            strip.cell(|ui| {
                ui.add_space(spacing_y / 2.0);
                let mut strip = StripBuilder::new(ui);
                let mut remaining_width = total_width;
                let mut col_count = 0;
                for i in 0..columns {
                    remaining_width -= if i > 0 { spacing_x } else { 0.0 };
                    strip = strip.size(Size::exact(width_per_column.min(remaining_width)));
                    remaining_width -= width_per_column;
                    col_count += 1;
                    if remaining_width <= 0.0 {
                        break;
                    }
                }
                strip = strip.clip(true);
                strip.horizontal(|mut strip| {
                    for i in 0..col_count {
                        strip.cell(|ui| {
                            ui.set_clip_rect(ui.available_rect_before_wrap());
                            super::player_list::frame::render(
                                ui,
                                pipe,
                                ui_state,
                                graphics,
                                full_ui_rect,
                                i * items_per_height
                                    ..((i + 1) * items_per_height).min(player_count),
                                height_of_item,
                            );
                        });
                    }
                });
            });
            strip.empty();
        });
}
