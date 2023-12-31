use egui::{epaint::RectShape, Color32, Layout, RichText, Rounding};
use graphics::graphics::Graphics;
use ui_base::{
    types::{UIPipe, UIState},
    utils::add_horizontal_margins,
};

use crate::scoreboard::user_data::UserData;

pub enum TopBarTypes {
    Neutral,
    Red,
    Blue,
    Spectator,
}

/// can contain various information
/// depends on the modification
/// map name, team name, differently colored
/// current team score, best player time
/// spectator info etc.
pub fn render(
    ui: &mut egui::Ui,
    _pipe: &mut UIPipe<UserData>,
    _ui_state: &mut UIState,
    _graphics: &mut Graphics,
    ty: TopBarTypes,
    rounding: Rounding,
) {
    ui.painter().add(RectShape::filled(
        ui.available_rect_before_wrap(),
        rounding,
        match ty {
            TopBarTypes::Neutral => Color32::DARK_GRAY,
            TopBarTypes::Red => Color32::DARK_RED,
            TopBarTypes::Blue => Color32::DARK_BLUE,
            TopBarTypes::Spectator => Color32::from_rgb(80, 30, 100),
        },
    ));
    const FONT_SIZE: f32 = 18.0;
    match ty {
        TopBarTypes::Neutral | TopBarTypes::Red | TopBarTypes::Spectator => {
            ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                add_horizontal_margins(ui, |ui| {
                    ui.label(
                        RichText::new("Team name")
                            .size(FONT_SIZE)
                            .color(Color32::WHITE),
                    );
                });
            });
        }
        TopBarTypes::Blue => {
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                add_horizontal_margins(ui, |ui| {
                    ui.label(
                        RichText::new("Team name")
                            .size(FONT_SIZE)
                            .color(Color32::WHITE),
                    );
                });
            });
        }
    }
}
