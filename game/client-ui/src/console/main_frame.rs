use egui::{text::LayoutJob, Color32, Pos2, Rect, Stroke, Style, TextFormat, Vec2};
use egui_extras::{Size, StripBuilder};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use ui_base::{
    style::default_style,
    types::{UIPipe, UIState},
};

use super::user_data::UserData;

fn console_style() -> Style {
    let mut style = default_style();
    style.visuals.extreme_bg_color = Color32::TRANSPARENT;
    style.visuals.widgets.inactive.bg_stroke = Stroke::NONE;
    //style.visuals.widgets.inactive.fg_stroke = Stroke::NONE;
    style.visuals.widgets.hovered.bg_stroke = Stroke::NONE;
    //style.visuals.widgets.hovered.fg_stroke = Stroke::NONE;
    style.visuals.widgets.active.bg_stroke = Stroke::NONE;
    //style.visuals.widgets.active.fg_stroke = Stroke::NONE;
    style.visuals.widgets.open.bg_stroke = Stroke::NONE;
    //style.visuals.widgets.open.fg_stroke = Stroke::NONE;
    //style.visuals.selection.stroke = Stroke::NONE;
    style.override_text_style = Some(egui::TextStyle::Monospace);
    style
}

/// square, fills most of the screen
pub fn render<B: GraphicsBackendInterface>(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    ui_state: &mut UIState,
    graphics: &mut GraphicsBase<B>,
    main_frame_only: bool,
) {
    ui.set_style(console_style());
    let width = ui.available_width();
    let height = ui.available_height() * 2.0 / 3.0;

    if main_frame_only {
        ui.painter().rect(
            Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(width, height)),
            0.0,
            Color32::from_rgb(255, 255, 255),
            Stroke::NONE,
        );
    } else {
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            ui_state.is_ui_open = false;
        }

        let mut has_text_selection = false;
        ui.allocate_ui_at_rect(
            Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(width, height)),
            |ui| {
                StripBuilder::new(ui)
                    .size(Size::exact(0.0))
                    .size(Size::remainder())
                    .size(Size::exact(15.0))
                    .size(Size::exact(20.0))
                    .size(Size::exact(0.0))
                    .vertical(|mut strip| {
                        strip.empty();
                        strip.cell(|ui| {
                            super::console_list::render(
                                ui,
                                pipe,
                                ui_state,
                                graphics,
                                &mut has_text_selection,
                            );
                        });
                        strip.cell(|ui| {
                            if !pipe.user_data.msg.is_empty() {
                                // add suggestions
                                let matcher = SkimMatcherV2::default();
                                let mut found_entries: Vec<(usize, (i64, Vec<usize>))> = pipe
                                    .user_data
                                    .entries
                                    .iter()
                                    .enumerate()
                                    .map(|(index, e)| {
                                        (
                                            index,
                                            matcher.fuzzy_indices(&e.full_name, pipe.user_data.msg),
                                        )
                                    })
                                    .filter(|(_, m)| m.is_some())
                                    .map(|(index, m)| (index, m.unwrap()))
                                    .collect();
                                found_entries.sort_by(|(_, (score_a, _)), (_, (score_b, _))| {
                                    score_b.cmp(score_a)
                                });
                                ui.horizontal(|ui| {
                                    for (entry_index, (_, matching_char_indices)) in found_entries {
                                        let msg_chars = pipe.user_data.entries[entry_index]
                                            .full_name
                                            .char_indices();
                                        let default_color = if ui.visuals().dark_mode {
                                            Color32::LIGHT_GRAY
                                        } else {
                                            Color32::DARK_GRAY
                                        };
                                        //ui.label(&pipe.user_data.entries[entry_index].full_name);
                                        let mut text_label = LayoutJob::default();
                                        for (i, msg_char) in msg_chars {
                                            if matching_char_indices.contains(&i) {
                                                text_label.append(
                                                    &msg_char.to_string(),
                                                    0.0,
                                                    TextFormat {
                                                        color: Color32::from_rgb(128, 128, 255),
                                                        ..Default::default()
                                                    },
                                                );
                                            } else {
                                                text_label.append(
                                                    &msg_char.to_string(),
                                                    0.0,
                                                    TextFormat {
                                                        color: default_color,
                                                        ..Default::default()
                                                    },
                                                );
                                            }
                                        }
                                        ui.label(text_label);
                                    }
                                });
                            }
                        });
                        strip.cell(|ui| {
                            super::input::render(ui, pipe, ui_state, graphics, has_text_selection);
                        });
                        strip.empty();
                    });
            },
        );
    }
}
