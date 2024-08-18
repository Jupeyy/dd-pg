use std::time::Duration;

use egui::{Color32, FontId, Frame, Layout, Margin, RichText, Rounding};

use egui_extras::{Size, StripBuilder};
use game_interface::types::render::game::{game_match::MatchStandings, GameRenderInfo};
use ui_base::{
    better_frame::BetterFrame,
    types::{UiRenderPipe, UiState},
};

use super::user_data::UserData;

/// not required
pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>, main_frame_only: bool) {
    ui.add_space(5.0);
    ui.set_clip_rect(ui.available_rect_before_wrap());
    if main_frame_only {
        // we don't need this
    } else {
        let tick_time_nanos =
            Duration::from_secs(1).as_nanos() as u64 / pipe.user_data.ticks_per_second.get();
        let secs = *pipe.user_data.race_timer_counter / pipe.user_data.ticks_per_second.get();
        let nanos = (*pipe.user_data.race_timer_counter % pipe.user_data.ticks_per_second.get())
            * tick_time_nanos;
        let race_time = Duration::new(secs, nanos as u32);

        let days = race_time.as_secs() / (3600 * 24);
        let ms = race_time.subsec_millis();
        let seconds = race_time.as_secs() % 60;
        let minutes = (race_time.as_secs() / 60) % 60;
        let hours = ((race_time.as_secs() / 60) / 60) % 24;
        let time_str = format!(
            "{}{}{:0>2}:{:0>2}:{:0>2}",
            if days > 0 {
                format!("{}d ", days)
            } else {
                String::default()
            },
            if hours > 0 || days > 0 {
                format!("{:0>2}:", hours)
            } else {
                String::default()
            },
            minutes,
            seconds,
            ms / 10
        );

        let rounding = 5.0;
        let margin = Margin::same(2.5);
        let color_a = |color: Color32, a: u8| {
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), a)
        };

        ui.vertical(|ui| {
            ui.style_mut().spacing.item_spacing.y = 0.0;
            let rect = ui
                .vertical_centered(|ui| {
                    let mut frame = Frame::default()
                        .rounding(Rounding {
                            ne: rounding,
                            nw: rounding,
                            ..Default::default()
                        })
                        .fill(color_a(Color32::BLACK, 50))
                        .inner_margin(margin)
                        .begin_better(ui);
                    let rect = frame
                        .content_ui
                        .label(
                            RichText::new(time_str)
                                .font(FontId::proportional(20.0))
                                .color(Color32::WHITE),
                        )
                        .rect;
                    frame.end(ui, rect);
                    rect
                })
                .inner;

            ui.vertical_centered(|ui| {
                // no spacing for points
                ui.style_mut().spacing.item_spacing = Default::default();
                ui.allocate_ui(
                    rect.expand((margin.left + margin.right) / 2.0).size(),
                    |ui| {
                        match pipe.user_data.game {
                            Some(GameRenderInfo::Match { standings }) => match standings {
                                MatchStandings::Solo { leading_players } => {
                                    todo!();
                                }
                                MatchStandings::Team {
                                    score_red,
                                    score_blue,
                                } => {
                                    StripBuilder::new(ui)
                                        .size(Size::remainder())
                                        .size(Size::remainder())
                                        .cell_layout(Layout::top_down(egui::Align::Center))
                                        .horizontal(|mut strip| {
                                            strip.cell(|ui| {
                                                Frame::none()
                                                    .fill(color_a(Color32::RED, 150))
                                                    .rounding(Rounding {
                                                        sw: rounding,
                                                        ..Default::default()
                                                    })
                                                    .show(ui, |ui| {
                                                        ui.colored_label(
                                                            Color32::WHITE,
                                                            format!("{}", score_red),
                                                        );
                                                    });
                                            });
                                            strip.cell(|ui| {
                                                Frame::none()
                                                    .fill(color_a(Color32::BLUE, 150))
                                                    .rounding(Rounding {
                                                        se: rounding,
                                                        ..Default::default()
                                                    })
                                                    .show(ui, |ui| {
                                                        ui.colored_label(
                                                            Color32::WHITE,
                                                            format!("{}", score_blue),
                                                        );
                                                    });
                                            });
                                        });
                                }
                            },
                            Some(GameRenderInfo::Race { .. }) => todo!(),
                            None => {
                                // don't render anything
                            }
                        }
                    },
                );
            });
        });
    }
}
