use egui::Layout;
use graphics::graphics::GraphicsBase;
use graphics_backend_traits::traits::GraphicsBackendInterface;
use ui_base::types::{UIPipe, UIState};

use super::user_data::UserData;

/// frame for the console entries
pub fn render<B: GraphicsBackendInterface>(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    _ui_state: &mut UIState,
    _graphics: &mut GraphicsBase<B>,
    has_text_selection: &mut bool,
) {
    egui::ScrollArea::vertical()
        .stick_to_bottom(true)
        .show(ui, |ui| {
            ui.with_layout(
                Layout::left_to_right(egui::Align::Min)
                    .with_main_justify(true)
                    .with_main_align(egui::Align::Min),
                |ui| {
                    let mut text_as_str = pipe.user_data.msgs.as_str();
                    let text = egui::TextEdit::multiline(&mut text_as_str)
                        .id_source("console-text-output")
                        .frame(false)
                        .show(ui);
                    *has_text_selection = text.cursor_range.is_some();
                },
            );
        });
}
