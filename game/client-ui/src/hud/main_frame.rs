use std::{borrow::Borrow, time::Duration};

use base::duration_ext::DurationToRaceStr;
use egui::{
    Align2, Color32, FontId, Frame, Layout, Margin, Rect, RichText, Rounding, UiBuilder, Vec2,
    Window,
};

use egui_extras::{Size, StripBuilder};
use game_interface::types::render::{
    character::TeeEye,
    game::{
        game_match::{LeadingCharacter, MatchStandings},
        GameRenderInfo,
    },
};
use math::math::vector::vec2;
use ui_base::{
    better_frame::BetterFrame,
    types::{UiRenderPipe, UiState},
};

use crate::utils::render_tee_for_ui;

use super::user_data::UserData;

/// not required
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    main_frame_only: bool,
) {
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

        let time_str = race_time.to_race_string();

        let color_a = |color: Color32, a: u8| {
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), a)
        };

        const ROUNDING: f32 = 5.0;
        const MARGIN: f32 = 2.5;
        let (max_height, rounding) = match pipe.user_data.game {
            Some(GameRenderInfo::Match {
                standings: MatchStandings::Solo { .. },
            }) => (60.0, Rounding::same(0.0)),
            Some(GameRenderInfo::Match {
                standings: MatchStandings::Sided { .. },
            }) => (
                40.0,
                Rounding {
                    ne: ROUNDING,
                    nw: ROUNDING,
                    ..Default::default()
                },
            ),
            Some(GameRenderInfo::Race { .. }) | None => (25.0, Rounding::same(ROUNDING)),
        };

        enum Side {
            Left,
            Right,
            Bottom(Rect),
        }
        let render_side = |pipe: &mut UiRenderPipe<UserData>,
                           ui: &mut egui::Ui,
                           ui_state: &mut UiState,
                           side: Side| {
            let rect = ui.available_rect_before_wrap();
            match pipe.user_data.game {
                Some(GameRenderInfo::Match { standings }) => match standings {
                    MatchStandings::Solo { leading_characters } => {
                        let mut render_char =
                            |leading_character: &Option<LeadingCharacter>, left: bool| {
                                let rounding = if left {
                                    Rounding {
                                        sw: ROUNDING,
                                        nw: ROUNDING,
                                        ..Default::default()
                                    }
                                } else {
                                    Rounding {
                                        ne: ROUNDING,
                                        se: ROUNDING,
                                        ..Default::default()
                                    }
                                };

                                let mut rect = rect;
                                rect.set_width(100.0);
                                rect.set_height(60.0);
                                ui.style_mut().spacing.item_spacing.y = 0.0;
                                ui.allocate_new_ui(UiBuilder::new().max_rect(rect), |ui| {
                                    Frame::default()
                                        .rounding(rounding)
                                        .fill(color_a(Color32::BLACK, 50))
                                        .inner_margin(Margin::same(MARGIN))
                                        .show(ui, |ui| {
                                            ui.set_height(60.0);
                                            ui.set_width(100.0);
                                            let data = &mut *pipe.user_data;
                                            if let Some((character, score)) = leading_character
                                                .as_ref()
                                                .and_then(|leading_character| {
                                                    data.character_infos
                                                        .get(&leading_character.character_id)
                                                        .map(|c| (c, leading_character.score))
                                                })
                                            {
                                                let rect = ui.available_rect_before_wrap();

                                                let tee_size =
                                                    rect.width().min(rect.height()).min(30.0);
                                                render_tee_for_ui(
                                                    data.canvas_handle,
                                                    data.skin_container,
                                                    data.skin_renderer,
                                                    ui,
                                                    ui_state,
                                                    ui.ctx().screen_rect(),
                                                    Some(rect),
                                                    character.info.skin.borrow(),
                                                    Some(&character.info.skin_info),
                                                    vec2::new(rect.center().x, rect.center().y),
                                                    tee_size,
                                                    TeeEye::Normal,
                                                );
                                                StripBuilder::new(ui)
                                                    .size(Size::remainder())
                                                    .size(Size::exact(tee_size))
                                                    .size(Size::remainder())
                                                    .cell_layout(
                                                        Layout::bottom_up(egui::Align::Center)
                                                            .with_main_align(egui::Align::Max),
                                                    )
                                                    .vertical(|mut strip| {
                                                        strip.cell(|ui| {
                                                            ui.colored_label(
                                                                Color32::WHITE,
                                                                character.info.name.as_str(),
                                                            );
                                                        });
                                                        strip.empty();

                                                        strip.cell(|ui| {
                                                            ui.with_layout(
                                                                Layout::bottom_up(
                                                                    egui::Align::Center,
                                                                )
                                                                .with_main_justify(false),
                                                                |ui| {
                                                                    ui.colored_label(
                                                                        Color32::WHITE,
                                                                        format!("{}", score),
                                                                    );
                                                                },
                                                            );
                                                        });
                                                    });
                                            }
                                        });
                                });
                            };

                        if matches!(side, Side::Left) {
                            render_char(&leading_characters[0], true);
                        }

                        if matches!(side, Side::Right) {
                            render_char(&leading_characters[1], false);
                        }
                    }
                    MatchStandings::Sided {
                        score_red,
                        score_blue,
                    } => {
                        if let Side::Bottom(rect) = side {
                            // no spacing for points
                            ui.style_mut().spacing.item_spacing = Default::default();
                            ui.allocate_ui(rect.expand(MARGIN).size(), |ui| {
                                StripBuilder::new(ui)
                                    .size(Size::remainder())
                                    .size(Size::remainder())
                                    .cell_layout(Layout::top_down(egui::Align::Center))
                                    .horizontal(|mut strip| {
                                        strip.cell(|ui| {
                                            Frame::none()
                                                .fill(color_a(Color32::RED, 150))
                                                .rounding(Rounding {
                                                    sw: ROUNDING,
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
                                                    se: ROUNDING,
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
                            });
                        }
                    }
                },
                Some(GameRenderInfo::Race { .. }) => {}
                None => {
                    // don't render anything
                }
            }
        };

        Window::new("")
            .resizable(false)
            .title_bar(false)
            .frame(Frame::none())
            .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 5.0))
            .max_height(max_height)
            .show(ui.ctx(), |ui| {
                ui.style_mut().spacing.item_spacing.y = 0.0;
                let rect = ui
                    .with_layout(
                        Layout::left_to_right(egui::Align::Center)
                            .with_main_justify(false)
                            .with_cross_justify(true),
                        |ui| {
                            render_side(pipe, ui, ui_state, Side::Left);

                            let mut frame = Frame::default()
                                .rounding(rounding)
                                .inner_margin(Margin::same(MARGIN))
                                .fill(color_a(Color32::BLACK, 50))
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

                            render_side(pipe, ui, ui_state, Side::Right);
                            rect
                        },
                    )
                    .inner;
                render_side(pipe, ui, ui_state, Side::Bottom(rect));
            });
    }
}
