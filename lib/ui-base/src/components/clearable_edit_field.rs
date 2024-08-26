use egui::{vec2, Layout, Response, Rounding};
use egui_extras::{Size, StripBuilder};

use super::menu_top_button::text_icon;

pub fn clearable_edit_field(
    ui: &mut egui::Ui,
    text: &mut String,
    input_at_most_size: Option<f32>,
) -> Option<Response> {
    let address = text;
    let style = ui.style_mut();
    let rounding = style.visuals.widgets.inactive.rounding.ne;
    style.spacing.item_spacing = vec2(0.0, 0.0);
    let mut res = None;
    ui.horizontal(|ui| {
        if let Some(input_at_most_size) = input_at_most_size {
            ui.set_max_width(input_at_most_size)
        }
        StripBuilder::new(ui)
            .size(Size::remainder())
            .size(Size::exact(20.0))
            .clip(true)
            .horizontal(|mut strip| {
                strip.cell(|ui| {
                    let style = ui.style_mut();
                    style.visuals.widgets.inactive.rounding = Rounding {
                        nw: rounding,
                        sw: rounding,
                        ..Default::default()
                    };
                    style.visuals.widgets.active.rounding = style.visuals.widgets.inactive.rounding;
                    style.visuals.widgets.hovered.rounding =
                        style.visuals.widgets.inactive.rounding;
                    style.visuals.widgets.noninteractive.rounding =
                        style.visuals.widgets.inactive.rounding;
                    style.visuals.widgets.open.rounding = style.visuals.widgets.inactive.rounding;
                    res = Some(
                        ui.with_layout(
                            Layout::left_to_right(egui::Align::Center).with_main_justify(true),
                            |ui| ui.text_edit_singleline(address),
                        )
                        .inner,
                    );
                });
                strip.cell(|ui| {
                    let style = ui.style_mut();
                    style.visuals.widgets.inactive.rounding = Rounding {
                        ne: rounding,
                        se: rounding,
                        ..Default::default()
                    };
                    style.visuals.widgets.active.rounding = style.visuals.widgets.inactive.rounding;
                    style.visuals.widgets.hovered.rounding =
                        style.visuals.widgets.inactive.rounding;
                    style.visuals.widgets.noninteractive.rounding =
                        style.visuals.widgets.inactive.rounding;
                    style.visuals.widgets.open.rounding = style.visuals.widgets.inactive.rounding;
                    let icon_text = text_icon(ui, "\u{f00d}");
                    if ui.button(icon_text).clicked() {
                        address.clear();
                    }
                });
            });
    });
    res
}
