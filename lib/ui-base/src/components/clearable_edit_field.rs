use egui::{vec2, Button, Layout, Response, Rounding, Stroke, TextEdit};
use egui_extras::{Size, StripBuilder};

use crate::utils::icon_font_text_for_btn;

pub fn clearable_edit_field(
    ui: &mut egui::Ui,
    text: &mut String,
    input_at_most_size: Option<f32>,
    max_chars: Option<usize>,
) -> Option<Response> {
    let address = text;
    let style = ui.style_mut();
    let rounding = style.visuals.widgets.inactive.rounding.ne;
    style.spacing.item_spacing = vec2(0.0, 0.0);
    let mut res = None;
    ui.horizontal(|ui| {
        StripBuilder::new(ui)
            .size(if let Some(input_at_most_size) = input_at_most_size {
                Size::remainder().at_most(input_at_most_size)
            } else {
                Size::remainder()
            })
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
                            |ui| {
                                ui.add(
                                    TextEdit::singleline(address)
                                        .char_limit(max_chars.unwrap_or(usize::MAX).max(1)),
                                )
                            },
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
                    if ui
                        .add(
                            Button::new(icon_font_text_for_btn(ui, "\u{f00d}"))
                                .stroke(Stroke::NONE),
                        )
                        .clicked()
                    {
                        address.clear();
                    }
                });
            });
    });
    res
}
