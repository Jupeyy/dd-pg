use std::{collections::BTreeMap, time::Duration};

use egui::{Color32, Layout};
use game_interface::types::resource_key::{NetworkResourceKey, ResourceKey};
use math::math::vector::vec2;
use ui_base::{
    components::clearable_edit_field::clearable_edit_field,
    types::{UiRenderPipe, UiState},
};

use crate::{main_menu::user_data::UserData, utils::render_tee_for_ui};

pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>, ui_state: &mut UiState) {
    ui.with_layout(Layout::top_down(egui::Align::Min), |ui| {
        super::themes::list::render(ui, pipe, ui_state);
    });
}
