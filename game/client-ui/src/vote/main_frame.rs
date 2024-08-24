use std::time::Duration;

use egui::{pos2, vec2, Align2, Color32, FontId, Frame, Rect, Rounding};
use game_interface::{types::render::character::TeeEye, votes::Voted};
use math::math::vector::vec2;
use ui_base::types::{UiRenderPipe, UiState};

use crate::{utils::render_tee_for_ui, vote::user_data::VoteRenderData};

use super::user_data::{UserData, VoteRenderType};

/// not required
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    main_frame_only: bool,
) {
    let full_rect = ui.ctx().screen_rect();
    let mut rect = ui.ctx().screen_rect();

    // 15% + some etra offset for the hud
    let x_offset = 10.0;
    let y_offset = 15.0 * rect.height() / 100.0 + 50.0;

    // at most allow 20% of the height
    let max_height = (20.0 * rect.height() / 100.0).min(150.0);
    let max_width = 300.0;

    rect.set_left(x_offset);
    rect.set_top(y_offset);
    rect.set_height(max_height);
    rect.set_width(max_width);

    ui.allocate_ui_at_rect(rect, |ui| {
        let vote_rect = ui.available_rect_before_wrap();
        Frame::window(ui.style()).show(ui, |ui| {
            ui.set_min_width(vote_rect.width());
            ui.set_width(vote_rect.width());
            ui.set_min_height(vote_rect.height());
            ui.set_height(vote_rect.height());
            let vote = &pipe.user_data.vote_data;

            fn render_header(ui: &mut egui::Ui, text: &str, remaining_time: &Duration) {
                const HEADER_SIZE: f32 = 20.0;
                let rect = ui.available_rect_before_wrap();
                ui.painter().text(
                    rect.min,
                    Align2::LEFT_TOP,
                    text,
                    FontId::proportional(HEADER_SIZE),
                    Color32::WHITE,
                );
                let mut pos = rect.right_top();
                pos.y += HEADER_SIZE / 4.0;
                ui.painter().text(
                    pos,
                    Align2::RIGHT_TOP,
                    format!("Ends in: {:.2}s", remaining_time.as_secs_f32()),
                    FontId::proportional(HEADER_SIZE / 2.0),
                    Color32::WHITE,
                );
                ui.add_space(HEADER_SIZE);
            }

            fn render_footer(ui: &mut egui::Ui, vote: &VoteRenderData, vote_rect: &Rect) {
                // extra margin
                ui.add_space(5.0);

                const VOTE_BAR_HEIGHT: f32 = 20.0;

                let max = vote.data.allowed_to_vote_count.max(1);
                let yes_perc = vote.data.yes_votes.clamp(0, max) as f32 / max as f32;
                let no_perc = vote.data.no_votes.clamp(0, max) as f32 / max as f32;

                let ui_rect = ui.available_rect_before_wrap();
                let result_y = ui_rect.center_top().y;
                let rect = Rect::from_center_size(
                    ui_rect.center_top(),
                    vec2(vote_rect.width(), VOTE_BAR_HEIGHT),
                );
                ui.painter()
                    .rect_filled(rect, Rounding::same(5.0), Color32::DARK_GRAY);

                if no_perc > 0.0 {
                    // no
                    let no_size = vote_rect.width() * no_perc;
                    let mut at = ui_rect.right_top();
                    at.x -= no_size / 2.0;
                    at.y = result_y;
                    let rect = Rect::from_center_size(at, vec2(no_size, VOTE_BAR_HEIGHT));
                    ui.painter().rect_filled(
                        rect,
                        Rounding {
                            ne: 5.0,
                            se: 5.0,
                            ..Default::default()
                        },
                        Color32::RED,
                    );
                    at.x -= no_size / 2.0 - 5.0;
                    ui.painter().text(
                        at,
                        egui::Align2::LEFT_CENTER,
                        format!("{:.1}%", no_perc * 100.0),
                        FontId::default(),
                        Color32::WHITE,
                    );
                }

                if yes_perc > 0.0 {
                    // yes
                    let yes_size = vote_rect.width() * yes_perc;
                    let mut at = ui_rect.left_top();
                    at.x += yes_size / 2.0;
                    at.y = result_y;
                    let rect = Rect::from_center_size(at, vec2(yes_size, VOTE_BAR_HEIGHT));
                    ui.painter().rect_filled(
                        rect,
                        Rounding {
                            nw: 5.0,
                            sw: 5.0,
                            ..Default::default()
                        },
                        Color32::GREEN,
                    );
                    at.x += yes_size / 2.0 - 5.0;
                    ui.painter().text(
                        at,
                        egui::Align2::RIGHT_CENTER,
                        format!("{:.1}%", yes_perc * 100.0),
                        FontId::default(),
                        Color32::BLACK,
                    );
                }

                ui.add_space(VOTE_BAR_HEIGHT);

                let rect = ui.available_rect_before_wrap();
                ui.painter().text(
                    rect.left_top(),
                    Align2::LEFT_TOP,
                    "f3 - vote yes",
                    FontId::default(),
                    if matches!(vote.voted, Some(Voted::Yes)) {
                        Color32::GREEN
                    } else {
                        Color32::LIGHT_GREEN
                    },
                );
                ui.painter().text(
                    rect.right_top(),
                    Align2::RIGHT_TOP,
                    "f4 - vote no",
                    FontId::default(),
                    if matches!(vote.voted, Some(Voted::No)) {
                        Color32::RED
                    } else {
                        Color32::LIGHT_RED
                    },
                );
            }

            const CONTENT_SIZE: f32 = 90.0;

            match vote.ty {
                VoteRenderType::Map(map) => {
                    render_header(
                        ui,
                        &format!("Vote map {}", map.name.as_str()),
                        vote.remaining_time,
                    );

                    ui.add_space(CONTENT_SIZE);

                    render_footer(ui, vote, &vote_rect);
                }
                VoteRenderType::PlayerVoteSpec(player) | VoteRenderType::PlayerVoteKick(player) => {
                    let is_kick = matches!(vote.ty, VoteRenderType::PlayerVoteKick(_));

                    render_header(
                        ui,
                        &format!("Vote {} player", if is_kick { "kick" } else { "spec" }),
                        vote.remaining_time,
                    );

                    let rect = ui.available_rect_before_wrap();
                    render_tee_for_ui(
                        pipe.user_data.canvas_handle,
                        pipe.user_data.skin_container,
                        pipe.user_data.render_tee,
                        ui,
                        ui_state,
                        full_rect,
                        None,
                        player.skin,
                        Some(player.skin_info),
                        vec2::new(
                            rect.min.x + CONTENT_SIZE / 2.0,
                            rect.min.y + CONTENT_SIZE / 2.0,
                        ),
                        CONTENT_SIZE,
                        TeeEye::Blink,
                    );
                    ui.painter().text(
                        pos2(rect.min.x + CONTENT_SIZE, rect.min.y + CONTENT_SIZE / 2.0),
                        Align2::LEFT_CENTER,
                        player.name,
                        FontId::proportional(22.0),
                        Color32::WHITE,
                    );

                    ui.add_space(CONTENT_SIZE);

                    render_footer(ui, vote, &vote_rect);
                }
            }
        });
    });
}
