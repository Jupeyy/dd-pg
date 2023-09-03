use egui::Color32;
use egui_extras::{Size, StripBuilder};
use graphics_backend::{backend::GraphicsBackend, types::Graphics};
use ui_traits::traits::UIRenderCallbackFunc;
use ui_wasm_manager::UIWinitWrapper;

pub struct DemoPage {}

impl DemoPage {
    pub fn new() -> Self {
        Self {}
    }
}

impl UIRenderCallbackFunc<UIWinitWrapper, GraphicsBackend> for DemoPage {
    fn render(
        &mut self,
        ui: &mut egui::Ui,
        _pipe: &mut ui_base::types::UIPipe<UIWinitWrapper>,
        _ui_state: &mut ui_base::types::UIState<UIWinitWrapper>,
        _graphics: &mut Graphics,
    ) {
        let dark_mode = ui.visuals().dark_mode;
        let faded_color = ui.visuals().window_fill();
        let faded_color = |color: Color32| -> Color32 {
            use egui::Rgba;
            let t = if dark_mode { 0.95 } else { 0.8 };
            egui::lerp(Rgba::from(color)..=Rgba::from(faded_color), t).into()
        };

        StripBuilder::new(ui)
            .size(Size::exact(50.0))
            .size(Size::remainder())
            .size(Size::relative(0.5).at_least(60.0))
            .size(Size::exact(10.0))
            .vertical(|mut strip| {
                strip.cell(|ui| {
                    ui.painter().rect_filled(
                        ui.available_rect_before_wrap(),
                        0.0,
                        faded_color(Color32::BLUE),
                    );
                    ui.label("width: 100%\nheight: 50px");
                });
                strip.strip(|builder| {
                    builder.sizes(Size::remainder(), 2).horizontal(|mut strip| {
                        strip.cell(|ui| {
                            ui.painter().rect_filled(
                                ui.available_rect_before_wrap(),
                                0.0,
                                faded_color(Color32::RED),
                            );
                            ui.label("width: 50%\nheight: remaining");
                        });
                        strip.strip(|builder| {
                            builder.sizes(Size::remainder(), 3).vertical(|mut strip| {
                                strip.empty();
                                strip.cell(|ui| {
                                    ui.painter().rect_filled(
                                        ui.available_rect_before_wrap(),
                                        0.0,
                                        faded_color(Color32::YELLOW),
                                    );
                                    ui.label("width: 50%\nheight: 1/3 of the red region");
                                });
                                strip.empty();
                            });
                        });
                    });
                });
                strip.strip(|builder| {
                    builder
                        .size(Size::remainder())
                        .size(Size::exact(120.0))
                        .size(Size::remainder())
                        .size(Size::exact(70.0))
                        .horizontal(|mut strip| {
                            strip.empty();
                            strip.strip(|builder| {
                                builder
                                    .size(Size::remainder())
                                    .size(Size::exact(60.0))
                                    .size(Size::remainder())
                                    .vertical(|mut strip| {
                                        strip.empty();
                                        strip.cell(|ui| {
                                            ui.painter().rect_filled(
                                                ui.available_rect_before_wrap(),
                                                0.0,
                                                faded_color(Color32::GOLD),
                                            );
                                            ui.label("width: 120px\nheight: 60px");
                                        });
                                    });
                            });
                            strip.empty();
                            strip.cell(|ui| {
                                ui.painter().rect_filled(
                                    ui.available_rect_before_wrap(),
                                    0.0,
                                    faded_color(Color32::GREEN),
                                );
                                ui.label("width: 70px\n\nheight: 50%, but at least 60px.");
                            });
                        });
                });
                strip.cell(|ui| {
                    ui.vertical_centered(|_ui| {});
                });
            });
    }
}
