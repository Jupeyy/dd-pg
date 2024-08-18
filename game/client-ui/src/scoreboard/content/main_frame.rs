use egui::{epaint::RectShape, Color32, Rect, Rounding, Shape};
use egui_extras::{Size, StripBuilder};

use game_interface::types::{
    game::GameEntityId,
    render::{
        character::CharacterInfo,
        scoreboard::{ScoreboardCharacterInfo, ScoreboardGameType},
    },
};
use hashlink::LinkedHashMap;
use ui_base::types::{UiRenderPipe, UiState};

use crate::scoreboard::user_data::UserData;

use super::topbar::TopBarTypes;

fn render_scoreboard_frame(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    full_ui_rect: Rect,
    topbar_type: TopBarTypes,
    rounding: Rounding,
    character_infos: &LinkedHashMap<GameEntityId, CharacterInfo>,
    players: &[ScoreboardCharacterInfo],
    top_label: &str,
    bottom_label: &str,
) {
    ui.painter().add(Shape::Rect(RectShape::filled(
        ui.available_rect_before_wrap(),
        rounding,
        Color32::from_rgba_unmultiplied(0, 0, 0, 100),
    )));
    StripBuilder::new(ui)
        .size(Size::exact(30.0))
        .size(Size::exact(0.0))
        .size(Size::remainder())
        .size(Size::exact(0.0))
        .size(Size::exact(10.0))
        .size(Size::exact(2.0))
        .vertical(|mut strip| {
            strip.cell(|ui| {
                super::topbar::render(ui, topbar_type, rounding, top_label);
            });
            strip.empty();
            strip.cell(|ui| {
                super::list::list::render(
                    ui,
                    pipe,
                    ui_state,
                    &full_ui_rect,
                    character_infos,
                    players,
                );
            });
            strip.empty();
            strip.cell(|ui| {
                super::footer::render(ui, bottom_label);
            });
            strip.empty();
        });
}

/// big boxes, rounded edges
pub fn render_players(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    main_frame_only: bool,
    full_ui_rect: Rect,
) -> f32 {
    let mut res = 0.0;
    let character_infos = pipe.user_data.character_infos;
    let scoreboard = &pipe.user_data.scoreboard;
    let options = &scoreboard.options;
    match &scoreboard.game {
        ScoreboardGameType::TeamPlay {
            red_characters: red_players,
            blue_characters: blue_players,
            red_team_name,
            blue_team_name,
            ..
        } => {
            StripBuilder::new(ui)
                .size(Size::exact(10.0))
                .size(Size::remainder())
                .size(Size::remainder())
                .size(Size::exact(10.0))
                .horizontal(|mut strip| {
                    strip.empty();
                    strip.cell(|ui| {
                        res = ui.available_width();
                        let rounding = Rounding {
                            nw: 5.0,
                            ..Default::default()
                        };
                        if main_frame_only {
                            ui.painter().add(Shape::Rect(RectShape::filled(
                                ui.available_rect_before_wrap(),
                                rounding,
                                Color32::from_rgba_unmultiplied(0, 0, 0, 255),
                            )));
                        } else {
                            render_scoreboard_frame(
                                ui,
                                pipe,
                                ui_state,
                                full_ui_rect,
                                TopBarTypes::Red,
                                rounding,
                                character_infos,
                                red_players,
                                &red_team_name,
                                &format!("Score limit: {}", options.score_limit),
                            );
                        }
                    });
                    strip.cell(|ui| {
                        let rounding = Rounding {
                            ne: 5.0,
                            ..Default::default()
                        };
                        if main_frame_only {
                            ui.painter().add(Shape::Rect(RectShape::filled(
                                ui.available_rect_before_wrap(),
                                rounding,
                                Color32::from_rgba_unmultiplied(0, 0, 0, 255),
                            )));
                        } else {
                            render_scoreboard_frame(
                                ui,
                                pipe,
                                ui_state,
                                full_ui_rect,
                                TopBarTypes::Blue,
                                rounding,
                                character_infos,
                                blue_players,
                                &blue_team_name,
                                &format!("Map: {}", options.map_name.as_str()),
                            );
                        }
                    });
                    strip.empty();
                });
        }
        ScoreboardGameType::SoloPlay {
            characters: players,
            ..
        } => {
            res = ui.available_width();
            let mut strip = StripBuilder::new(ui);

            let split_count = if players.len() > 16 { 2 } else { 1 };

            strip = strip.size(Size::exact(10.0));
            for _ in 0..split_count {
                strip = strip.size(Size::remainder());
            }
            strip = strip.size(Size::exact(10.0));
            strip.horizontal(|mut strip| {
                strip.empty();
                for i in 0..split_count {
                    let rounding = if i == 0 {
                        if split_count == 1 {
                            Rounding {
                                nw: 5.0,
                                ne: 5.0,
                                ..Default::default()
                            }
                        } else {
                            Rounding {
                                nw: 5.0,
                                ..Default::default()
                            }
                        }
                    } else {
                        Rounding {
                            ne: 5.0,
                            ..Default::default()
                        }
                    };
                    let players = if split_count > 1 {
                        if i == 0 {
                            &players[0..players.len() / 2]
                        } else {
                            &players[players.len() / 2..players.len()]
                        }
                    } else {
                        players.as_slice()
                    };
                    strip.cell(|ui| {
                        if main_frame_only {
                            ui.painter().add(Shape::Rect(RectShape::filled(
                                ui.available_rect_before_wrap(),
                                rounding,
                                Color32::from_rgba_unmultiplied(0, 0, 0, 255),
                            )));
                        } else {
                            render_scoreboard_frame(
                                ui,
                                pipe,
                                ui_state,
                                full_ui_rect,
                                TopBarTypes::Neutral,
                                rounding,
                                character_infos,
                                players,
                                &format!("Map: {}", options.map_name.as_str(),),
                                &format!("Score limit: {}", options.score_limit,),
                            );
                        }
                    });
                }
                strip.empty();
            });
        }
    }
    res
}

pub fn render_spectators(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    main_frame_only: bool,
    full_ui_rect: Rect,
) {
    let character_infos = pipe.user_data.character_infos;
    let scoreboard = &pipe.user_data.scoreboard;
    let spectator_players = match &scoreboard.game {
        ScoreboardGameType::TeamPlay {
            spectator_players, ..
        } => spectator_players,
        ScoreboardGameType::SoloPlay {
            spectator_players, ..
        } => spectator_players,
    };
    if spectator_players.is_empty() {
        return;
    }

    let rounding = Rounding {
        ..Default::default()
    };

    if main_frame_only {
        ui.painter().add(Shape::Rect(RectShape::filled(
            ui.available_rect_before_wrap(),
            rounding,
            Color32::from_rgba_unmultiplied(0, 0, 0, 255),
        )));
    } else {
        render_scoreboard_frame(
            ui,
            pipe,
            ui_state,
            full_ui_rect,
            TopBarTypes::Spectator,
            rounding,
            character_infos,
            spectator_players,
            "Spectators",
            "",
        );
    }
}
