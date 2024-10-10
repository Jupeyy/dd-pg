use egui::{epaint::Shadow, Color32};
use ui_base::{
    style::default_style,
    types::{UiRenderPipe, UiState},
};
use ui_traits::traits::UiPageInterface;

use super::{main_frame, user_data::UserData};

pub struct EditorUi {}

impl Default for EditorUi {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorUi {
    pub fn new() -> Self {
        Self {}
    }

    pub fn set_style(ui: &mut egui::Ui, main_frame: bool) {
        let mut style = default_style();
        let clr = style.visuals.window_fill.to_srgba_unmultiplied();
        style.visuals.window_fill = Color32::from_rgba_unmultiplied(clr[0], clr[1], clr[2], 180);
        let clr = style.visuals.panel_fill.to_srgba_unmultiplied();
        style.visuals.panel_fill = Color32::from_rgba_unmultiplied(clr[0], clr[1], clr[2], 225);
        if main_frame {
            style.visuals.window_shadow = Shadow::NONE;
            style.visuals.popup_shadow = Shadow::NONE;
        }
        style.interaction.show_tooltips_only_when_still = false;
        style.interaction.tooltip_delay = 0.0;
        ui.ctx().set_style(style);
        ui.reset_style();
    }
}

impl<'a> UiPageInterface<UserData<'a>> for EditorUi {
    fn has_blur(&self) -> bool {
        true
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<UserData>,
        _ui_state: &mut UiState,
    ) {
        Self::set_style(ui, true);
        main_frame::render(ui, pipe, true)
    }

    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<UserData>,
        _ui_state: &mut UiState,
    ) {
        Self::set_style(ui, false);
        main_frame::render(ui, pipe, false)
    }
}
