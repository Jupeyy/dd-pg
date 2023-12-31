use egui_extras::{Size, StripBuilder};
use graphics::graphics::Graphics;
use ui_base::types::{UIPipe, UIState};

use super::user_data::UserData;

/// top bar
/// big square, rounded edges
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    ui_state: &mut UIState,
    graphics: &mut Graphics,
    main_frame_only: bool,
) {
    StripBuilder::new(ui)
        .size(Size::exact(20.0))
        .size(Size::exact(10.0))
        .size(Size::remainder())
        .size(Size::exact(10.0))
        .vertical(|mut strip| {
            strip.cell(|ui| {
                super::topbar::main_frame::render(ui, pipe, ui_state, graphics, main_frame_only);
            });
            strip.empty();
            strip.strip(|builder| {
                builder
                    .size(Size::exact(10.0))
                    .size(Size::remainder())
                    .size(Size::exact(10.0))
                    .horizontal(|mut strip| {
                        strip.empty();
                        strip.cell(|ui| {
                            super::content::main_frame::render(
                                ui,
                                pipe,
                                ui_state,
                                graphics,
                                main_frame_only,
                            );
                        });
                        strip.empty();
                    });
            });
            strip.empty();
        });
}
