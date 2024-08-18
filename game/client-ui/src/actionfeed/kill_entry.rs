use client_types::actionfeed::ActionFeedKill;
use egui::{Color32, Layout, Rect};
use game_interface::types::{
    character_info::NetworkSkinInfo, render::character::TeeEye, resource_key::ResourceKey,
};
use math::math::vector::vec2;
use ui_base::types::{UiRenderPipe, UiState};

use crate::{actionfeed::shared::entry_frame, utils::render_tee_for_ui};

use super::user_data::UserData;

/// one actionfeed entry
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    kill: &ActionFeedKill,
    full_rect: &Rect,
) {
    entry_frame(ui, |ui| {
        let tee_size = 20.0;
        let margin_from_tee = 2.0;

        struct RenderTee {
            skin: ResourceKey,
            skin_info: NetworkSkinInfo,
            pos: vec2,
        }
        // TODO: some day, maybe don't make so many heap allocations
        let mut render_tees = vec![];

        let response = ui.with_layout(Layout::right_to_left(egui::Align::Min), |ui| {
            ui.style_mut().spacing.item_spacing.x = 4.0;
            ui.style_mut().spacing.item_spacing.y = 0.0;
            ui.horizontal(|ui| {
                for victim in &kill.victims {
                    ui.colored_label(Color32::WHITE, &victim.name);
                    ui.add_space(tee_size + margin_from_tee);
                    let rect = ui.available_rect_before_wrap();
                    render_tees.push(RenderTee {
                        skin: victim.skin.clone(),
                        skin_info: victim.skin_info,
                        pos: vec2::new(
                            rect.max.x + tee_size / 2.0 + margin_from_tee / 2.0,
                            rect.min.y + rect.height() / 2.0,
                        ),
                    });
                    ui.add_space(5.0);
                }
                if let Some(killer) = &kill.killer {
                    ui.colored_label(Color32::WHITE, &killer.name);
                    ui.add_space(tee_size + margin_from_tee);
                    let rect = ui.available_rect_before_wrap();
                    render_tees.push(RenderTee {
                        skin: killer.skin.clone(),
                        skin_info: killer.skin_info,
                        pos: vec2::new(
                            rect.max.x + tee_size / 2.0 + margin_from_tee / 2.0,
                            rect.min.y + rect.height() / 2.0,
                        ),
                    });
                    ui.add_space(5.0);
                }
            });
        });

        for render_tee in render_tees {
            render_tee_for_ui(
                pipe.user_data.canvas_handle,
                pipe.user_data.skin_container,
                pipe.user_data.render_tee,
                ui,
                ui_state,
                *full_rect,
                Some(ui.clip_rect()),
                &render_tee.skin,
                Some(&render_tee.skin_info),
                render_tee.pos,
                tee_size,
                TeeEye::Normal,
            );
        }
    });
}
