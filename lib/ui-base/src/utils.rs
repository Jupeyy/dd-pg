use egui::{Response, Ui};
use egui_extras::{Size, StripBuilder};

pub fn add_horizontal_margins(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) -> Response {
    StripBuilder::new(ui)
        .size(Size::exact(0.0))
        .size(Size::remainder())
        .size(Size::exact(0.0))
        .horizontal(|mut strip| {
            strip.empty();
            strip.cell(add_contents);
            strip.empty();
        })
}
