use api::graphics::graphics::{Graphics, GraphicsBackend};
use api_ui::UIWinitWrapper;
use egui_extras::{Size, StripBuilder};
use ui_base::types::{UIPipe, UIState};
use ui_traits::traits::UIRenderCallbackFunc;

pub struct ExamplePage {}

impl ExamplePage {
    pub fn new() -> Self {
        Self {}
    }
}

impl UIRenderCallbackFunc<UIWinitWrapper, GraphicsBackend> for ExamplePage {
    fn render(
        &mut self,
        ui: &mut egui::Ui,
        _pipe: &mut UIPipe<UIWinitWrapper>,
        _ui_state: &mut UIState<UIWinitWrapper>,
        _graphics: &mut Graphics,
    ) {
        /*if ui.button("I'm a example page...").clicked() {
            // do nothing
        }

        let mut quad_scope = graphics.quads_begin();
        quad_scope.map_canvas(0.0, 0.0, 200.0, 200.0);
        quad_scope.set_colors_from_single(1.0, 0.0, 0.0, 0.25);
        quad_scope.quads_draw_tl(&[CQuadItem::new(50.0, 0.0, 100.0, 100.0)]);*/
        StripBuilder::new(ui)
            .size(Size::exact(10.0))
            .size(Size::remainder())
            .vertical(|mut strip| {
                strip.cell(|ui| {
                    ui.menu_button("Test", |ui| {
                        if ui.button("Close menu").clicked() {
                            ui.close_menu();
                        };
                    });
                });
                strip.strip(|builder| {
                    builder
                        .size(Size::exact(10.0))
                        .size(Size::remainder())
                        .horizontal(|mut strip| {
                            strip.cell(|_ui| {});
                            strip.cell(|ui| {
                                egui::SidePanel::left("left_panel")
                                    .resizable(true)
                                    .default_width(150.0)
                                    .width_range(80.0..=200.0)
                                    .show_inside(ui, |ui| {
                                        ui.vertical_centered(|ui| {
                                            ui.heading("Left Panel");
                                        });
                                    });

                                egui::TopBottomPanel::bottom("bottom_panel")
                                    .resizable(true)
                                    .default_height(50.0)
                                    .height_range(20.0..=100.0)
                                    .show_inside(ui, |ui| {
                                        ui.vertical_centered(|ui| {
                                            ui.heading("Bottom Panel");
                                        });
                                    });

                                egui::CentralPanel::default().show_inside(ui, |ui| {
                                    ui.vertical_centered(|ui| {
                                        ui.heading("Central Panel");
                                    });
                                });
                            });
                        });
                });
            });
        ui.horizontal(|ui| {
            ui.label("test");
        });
    }
}
