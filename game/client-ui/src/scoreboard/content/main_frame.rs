use client_types::scoreboard::ScoreboardGameType;
use egui::{epaint::RectShape, Rect, Rounding, Shape};
use egui_extras::{Size, StripBuilder};
use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use ui_base::types::{UIPipe, UIState};

use crate::scoreboard::user_data::UserData;

use super::topbar::TopBarTypes;

fn render_scoreboard_frame<B: GraphicsBackendInterface>(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    ui_state: &mut UIState,
    graphics: &mut GraphicsBase<B>,
    full_ui_rect: Rect,
    topbar_type: TopBarTypes,
) {
    StripBuilder::new(ui)
        .size(Size::exact(30.0))
        .size(Size::exact(0.0))
        .size(Size::remainder())
        .size(Size::exact(0.0))
        .size(Size::exact(10.0))
        .size(Size::exact(2.0))
        .vertical(|mut strip| {
            strip.cell(|ui| {
                super::topbar::render(ui, pipe, ui_state, graphics, topbar_type);
            });
            strip.empty();
            strip.cell(|ui| {
                super::list::list::render(ui, pipe, ui_state, graphics, &full_ui_rect);
            });
            strip.empty();
            strip.cell(|ui| {
                super::footer::render(ui, pipe, ui_state, graphics);
            });
            strip.empty();
        });
}

/// big box, rounded edges
pub fn render<B: GraphicsBackendInterface>(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    ui_state: &mut UIState,
    graphics: &mut GraphicsBase<B>,
    main_frame_only: bool,
    full_ui_rect: Rect,
) {
    match pipe.user_data.game_data {
        ScoreboardGameType::TeamPlay {
            red_players,
            blue_players,
            spectator_players,
        } => {
            StripBuilder::new(ui)
                .size(Size::exact(10.0))
                .size(Size::remainder())
                .size(Size::remainder())
                .size(Size::exact(10.0))
                .horizontal(|mut strip| {
                    strip.empty();
                    strip.cell(|ui| {
                        if main_frame_only {
                            ui.painter().add(Shape::Rect(RectShape::filled(
                                ui.available_rect_before_wrap(),
                                Rounding {
                                    nw: 5.0,
                                    sw: 3.0,
                                    ..Default::default()
                                },
                                ui.style().visuals.window_fill,
                            )));
                        } else {
                            render_scoreboard_frame(
                                ui,
                                pipe,
                                ui_state,
                                graphics,
                                full_ui_rect,
                                TopBarTypes::Red,
                            );
                        }
                    });
                    strip.cell(|ui| {
                        if main_frame_only {
                            ui.painter().add(Shape::Rect(RectShape::filled(
                                ui.available_rect_before_wrap(),
                                Rounding {
                                    ne: 5.0,
                                    se: 3.0,
                                    ..Default::default()
                                },
                                ui.style().visuals.window_fill,
                            )));
                        } else {
                            render_scoreboard_frame(
                                ui,
                                pipe,
                                ui_state,
                                graphics,
                                full_ui_rect,
                                TopBarTypes::Blue,
                            );
                        }
                    });
                    strip.empty();
                });
        }
        ScoreboardGameType::SoloPlay {
            players,
            spectator_players,
        } => todo!(),
    }
}
