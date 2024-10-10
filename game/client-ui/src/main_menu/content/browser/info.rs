use egui::{Align, FontId, Layout};
use egui_extras::{Size, StripBuilder};
use ui_base::types::UiRenderPipe;

use crate::main_menu::user_data::UserData;

/// simply a label
pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>) {
    let server_count = pipe.user_data.browser_data.servers.len();
    let player_count = pipe
        .user_data
        .browser_data
        .servers
        .iter()
        .map(|server| server.info.players.len())
        .sum::<usize>();

    let font_size = 10.0;
    ui.style_mut().override_font_id = Some(FontId::proportional(font_size));
    ui.with_layout(
        Layout::top_down(Align::Max)
            .with_main_align(Align::Center)
            .with_main_justify(true),
        |ui| {
            ui.style_mut().spacing.item_spacing.y = 0.0;
            StripBuilder::new(ui)
                .size(Size::relative(0.5))
                .size(Size::relative(0.5))
                .vertical(|mut strip| {
                    strip.cell(|ui| {
                        ui.label(format!("{server_count} servers"));
                    });
                    strip.cell(|ui| {
                        ui.label(format!("{player_count} players"));
                    });
                });
        },
    );
}
