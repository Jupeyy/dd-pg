use egui::{epaint::RectShape, Color32, Rect};
use egui_extras::{Size, StripBuilder};

use game_interface::types::{
    game::GameEntityId,
    render::{character::CharacterInfo, scoreboard::ScoreboardCharacterInfo},
};
use hashlink::LinkedHashMap;
use ui_base::{
    types::{UiRenderPipe, UiState},
    utils::add_horizontal_margins,
};

use crate::scoreboard::user_data::UserData;

use super::definitions::{
    TABLE_CONTENT_COLUMN_SPACING, TABLE_CONTENT_ROW_HEIGHTS, TABLE_CONTENT_WIDTH,
    TABLE_FONT_SIZE_COUNT,
};

struct CalculatedFontSizeIndex {
    index: usize,
    columns: usize,
    only_min_width_fits: bool,
}

/// `spacing_x_column`: spacing between columns
/// `spacing_x`: spacing between two table "instances"
/// (if height is not enough to display all rows, we create a new table in the same view)
fn calc_font_size_index(
    width: f32,
    height: f32,
    spacing_x: f32,
    spacing_y: f32,
    num_rows: usize,
    cur_column_count: usize,
    cur_font_index: usize,
) -> CalculatedFontSizeIndex {
    let min_columns_to_display = 3;
    let spacing_x_column = TABLE_CONTENT_COLUMN_SPACING[cur_font_index];
    let min_table_width = TABLE_CONTENT_WIDTH[cur_font_index]
        .iter()
        .enumerate()
        .filter(|(index, _)| *index < min_columns_to_display)
        .map(|(_, e)| e)
        .sum::<f32>()
        + (spacing_x_column * (min_columns_to_display - 1) as f32);
    let max_table_width = TABLE_CONTENT_WIDTH[cur_font_index]
        .iter()
        .sum::<f32>()
        + (spacing_x_column * (TABLE_CONTENT_WIDTH[cur_font_index].len() - 1) as f32);

    let items_per_col = num_rows.div_ceil(cur_column_count) as f32;
    let spacing_of_items = spacing_y * items_per_col - spacing_y;
    let full_height = TABLE_CONTENT_ROW_HEIGHTS[cur_font_index] * items_per_col + spacing_of_items;

    let required_size_for_full =
        cur_column_count as f32 * max_table_width + (cur_column_count - 1) as f32 * spacing_x;
    let required_size_for_min =
        cur_column_count as f32 * min_table_width + (cur_column_count - 1) as f32 * spacing_x;

    if height < full_height && cur_font_index > 0 {
        calc_font_size_index(
            width,
            height,
            spacing_x_column,
            spacing_y,
            num_rows,
            cur_column_count,
            cur_font_index - 1,
        )
    } else {
        // font is already smallest
        let next_col_count = cur_column_count + 1;
        // check if a additional column would fit somehow
        let required_size =
            next_col_count as f32 * min_table_width + (next_col_count - 1) as f32 * spacing_x;
        if height < full_height && required_size <= width {
            calc_font_size_index(
                width,
                height,
                spacing_x_column,
                spacing_y,
                num_rows,
                next_col_count,
                TABLE_FONT_SIZE_COUNT - 1,
            )
        }
        // check if column would fit with the current font size
        else if required_size_for_min > width && cur_font_index > 0 {
            calc_font_size_index(
                width,
                height,
                spacing_x_column,
                spacing_y,
                num_rows,
                cur_column_count,
                cur_font_index - 1,
            )
        } else {
            let only_min_width_fits = required_size_for_full > width;
            // if only min width fits and we can decrease size, try that
            // generally prefering full width is nicer
            if only_min_width_fits && cur_font_index > 0 {
                let res = calc_font_size_index(
                    width,
                    height,
                    spacing_x_column,
                    spacing_y,
                    num_rows,
                    cur_column_count,
                    cur_font_index - 1,
                );
                // but if the result also only allows min width, we can also just keep the current
                if res.only_min_width_fits {
                    CalculatedFontSizeIndex {
                        index: cur_font_index,
                        columns: cur_column_count,
                        only_min_width_fits,
                    }
                } else {
                    res
                }
            } else {
                CalculatedFontSizeIndex {
                    index: cur_font_index,
                    columns: cur_column_count,
                    only_min_width_fits,
                }
            }
        }
    }
}

/// table header + player list
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    full_ui_rect: &Rect,
    character_infos: &LinkedHashMap<GameEntityId, CharacterInfo>,
    players: &[ScoreboardCharacterInfo],
) {
    let player_count = players.len();
    let height_header = 12.0;
    let spacing_x = ui.style().spacing.item_spacing.x;
    let spacing_y = ui.style().spacing.item_spacing.y;
    let margin_x = spacing_x * 2.0;
    let total_width = ui.available_width() - margin_x;

    let CalculatedFontSizeIndex {
        index: font_size_index,
        columns,
        ..
    } = calc_font_size_index(
        total_width,
        ui.available_height() - (spacing_y * 3.0 + height_header),
        spacing_x,
        spacing_y,
        player_count,
        1,
        TABLE_FONT_SIZE_COUNT - 1,
    );
    let items_per_height = player_count.div_ceil(columns);

    let width_per_column =
        ((total_width - spacing_x * (columns - 1) as f32) / columns as f32).max(5.0);

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
                    Color32::from_rgba_unmultiplied(70, 70, 70, 255),
                ));
                add_horizontal_margins(ui, |ui| {
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
                                super::header::render(ui, pipe, ui_state, font_size_index);
                            });
                        }
                    });
                });
            });
            strip.empty();
            strip.cell(|ui| {
                add_horizontal_margins(ui, |ui| {
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
                                    character_infos,
                                    players,
                                    full_ui_rect,
                                    i * items_per_height
                                        ..((i + 1) * items_per_height).min(player_count),
                                    font_size_index,
                                );
                            });
                        }
                    });
                });
            });
            strip.empty();
        });
}
