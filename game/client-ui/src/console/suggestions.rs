use client_types::console::ConsoleEntry;
use command_parser::parser::CommandsTyped;
use config::traits::ConfigInterface;
use egui::{
    epaint::Shadow, scroll_area::ScrollBarVisibility, text::LayoutJob, Color32, FontId, Frame,
    Margin, ScrollArea, TextFormat, UiBuilder,
};
use ui_base::types::UiRenderPipe;

use super::{user_data::UserData, utils::find_matches};

pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>, cmds: &CommandsTyped) {
    if !pipe.user_data.msg.is_empty() {
        // add suggestions
        let found_entries = find_matches(
            cmds,
            *pipe.user_data.cursor,
            pipe.user_data.entries,
            pipe.user_data.msg,
        );

        let mut rect = ui.available_rect_before_wrap();
        rect.min.x += 5.0;
        rect.max.x -= 5.0;
        let shadow_color = ui.style().visuals.window_shadow.color;
        ui.allocate_new_ui(UiBuilder::new().max_rect(rect), |ui| {
            ui.vertical(|ui| {
                let found_entries_is_empty = found_entries.is_empty();

                ScrollArea::horizontal()
                    .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for (entry_index, (_, matching_char_indices)) in found_entries {
                                let (bg_color_text, match_color, default_color, margin, shadow) =
                                    if *pipe.user_data.select_index == Some(entry_index) {
                                        (
                                            Color32::from_rgba_unmultiplied(140, 140, 140, 15),
                                            Color32::from_rgb(180, 180, 255),
                                            Color32::from_rgb(255, 255, 255),
                                            Margin::symmetric(5.0, 5.0),
                                            Shadow {
                                                blur: 10.0,
                                                spread: 1.0,
                                                color: shadow_color,
                                                ..Default::default()
                                            },
                                        )
                                    } else {
                                        (
                                            Color32::TRANSPARENT,
                                            Color32::from_rgb(180, 180, 255),
                                            if ui.visuals().dark_mode {
                                                Color32::WHITE
                                            } else {
                                                Color32::DARK_GRAY
                                            },
                                            Margin::symmetric(5.0, 5.0),
                                            Shadow::NONE,
                                        )
                                    };
                                let shorted_path = match &pipe.user_data.entries[entry_index] {
                                    ConsoleEntry::Var(v) => v
                                        .full_name
                                        .replace("$KEY$", "[key]")
                                        .replace("$INDEX$", "[index]"),
                                    ConsoleEntry::Cmd(c) => c.name.clone(),
                                };
                                let msg_chars = shorted_path.chars().enumerate();
                                //ui.label(&pipe.user_data.entries[entry_index].full_name);
                                let mut text_label = LayoutJob::default();
                                for (i, msg_char) in msg_chars {
                                    if matching_char_indices.contains(&i) {
                                        text_label.append(
                                            &msg_char.to_string(),
                                            0.0,
                                            TextFormat {
                                                color: match_color,
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
                                let label = Frame::default()
                                    .fill(bg_color_text)
                                    .rounding(5.0)
                                    .inner_margin(margin)
                                    .shadow(shadow)
                                    .show(ui, |ui| ui.label(text_label));
                                if *pipe.user_data.select_index == Some(entry_index) {
                                    label.inner.scroll_to_me(None);
                                }
                            }
                        });
                    });

                let selected_index = *pipe.user_data.select_index;
                let selected_entry = (!found_entries_is_empty)
                    .then_some(selected_index.and_then(|index| pipe.user_data.entries.get(index)))
                    .flatten();
                if let Some(selected_entry) = selected_entry {
                    let mut job = LayoutJob::default();
                    let font_size = 9.0;
                    match selected_entry {
                        ConsoleEntry::Var(v) => {
                            let config = &mut *pipe.user_data.config;
                            let val = config
                                .engine
                                .try_set_from_str(v.full_name.clone(), None, None, None, 0)
                                .map(Some)
                                .unwrap_or_else(|_| {
                                    config
                                        .game
                                        .try_set_from_str(v.full_name.clone(), None, None, None, 0)
                                        .map(Some)
                                        .ok()
                                        .flatten()
                                });

                            if let Some(mut val) = val {
                                if val.len() > 42 {
                                    val.truncate(39);
                                    val = format!("{val}...");
                                }
                                job.append(
                                    "current value: ",
                                    0.0,
                                    TextFormat {
                                        font_id: FontId::monospace(font_size),
                                        color: Color32::WHITE,
                                        ..Default::default()
                                    },
                                );
                                job.append(
                                    &val,
                                    0.0,
                                    TextFormat {
                                        font_id: FontId::monospace(font_size),
                                        color: Color32::WHITE,
                                        background: Color32::DARK_GRAY,
                                        ..Default::default()
                                    },
                                );
                                job.append(
                                    ", ",
                                    0.0,
                                    TextFormat {
                                        font_id: FontId::monospace(font_size),
                                        color: Color32::WHITE,
                                        ..Default::default()
                                    },
                                );
                            }

                            job.append(
                                &format!("usage: {}", v.usage),
                                0.0,
                                TextFormat {
                                    font_id: FontId::monospace(font_size),
                                    color: Color32::WHITE,
                                    ..Default::default()
                                },
                            );
                        }
                        ConsoleEntry::Cmd(cmd) => {
                            job.append(
                                &format!("usage: {}", cmd.usage),
                                0.0,
                                TextFormat {
                                    font_id: FontId::monospace(font_size),
                                    color: Color32::WHITE,
                                    ..Default::default()
                                },
                            );
                        }
                    }
                    ui.add_space(3.0);
                    ui.label(job);
                }
            });
        });
    }
}
