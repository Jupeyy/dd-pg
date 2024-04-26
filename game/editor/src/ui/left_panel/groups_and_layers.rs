use crate::ui::user_data::UserDataWithTab;
use crate::{
    map::{
        EditorCommonLayerOrGroupAttrInterface, EditorDesignLayerInterface, EditorGroup,
        EditorMapInterface, EditorMapSetGroup, EditorMapSetLayer, EditorResources,
    },
    ui::utils::{group_name, icon_font_text, layer_name, layer_name_phy},
};

use egui::{collapsing_header::CollapsingState, Button, Color32, Layout};
use egui_extras::Size;
use ui_base::types::{UIPipe, UIState};

fn button_selected_style() -> egui::Stroke {
    let mut res = egui::Stroke::default();
    res.width = 2.0;
    res.color = Color32::LIGHT_GREEN;
    res
}

pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserDataWithTab>,
    ui_state: &mut UIState,
    main_frame_only: bool,
) {
    let tab = &mut *pipe.user_data.editor_tab;
    let map = &mut tab.map;

    let mut activated_layer = None;
    let mut selected_layers = Vec::new();
    let mut selected_groups = Vec::new();
    let group_ui = |id: &str,
                    ui: &mut egui::Ui,
                    resources: &EditorResources,
                    groups: &mut Vec<EditorGroup>| {
        let mut activated_layer = None;
        let mut selected_layers = Vec::new();
        let mut selected_groups = Vec::new();
        for (g, group) in groups.iter_mut().enumerate() {
            CollapsingState::load_with_default_open(ui.ctx(), format!("{}-{g}", id).into(), true)
                .show_header(ui, |ui| {
                    ui.with_layout(Layout::right_to_left(egui::Align::Min), |ui| {
                        let hidden = group.editor_attr_mut().hidden;
                        let hide_btn = Button::new(if hidden {
                            icon_font_text(ui, "\u{f070}")
                        } else {
                            icon_font_text(ui, "\u{f06e}")
                        })
                        .selected(group.editor_attr().hidden)
                        .fill(ui.style().visuals.window_fill);
                        if ui.add(hide_btn).clicked() {
                            group.editor_attr_mut().hidden = !hidden;
                        }
                        ui.vertical_centered_justified(|ui| {
                            let btn = Button::new(group_name(group, g)).frame(false);
                            if ui.add(btn).secondary_clicked() {
                                selected_groups.push(g);
                            }
                        })
                    })
                })
                .body(|ui| {
                    for (l, layer) in group.layers.iter_mut().enumerate() {
                        let layer_btn = {
                            let mut btn = egui::Button::new(layer_name(&ui, resources, layer, l));
                            if layer.editor_attr().active {
                                btn = btn.selected(true);
                            }
                            if layer.is_selected() {
                                btn = btn.stroke(button_selected_style());
                            }
                            btn
                        };

                        ui.with_layout(Layout::right_to_left(egui::Align::Min), |ui| {
                            let hidden = layer.editor_attr_mut().hidden;
                            let hide_btn = Button::new(if hidden {
                                icon_font_text(ui, "\u{f070}")
                            } else {
                                icon_font_text(ui, "\u{f06e}")
                            })
                            .selected(layer.editor_attr().hidden);
                            if ui.add(hide_btn).clicked() {
                                layer.editor_attr_mut().hidden = !hidden;
                            }

                            ui.vertical_centered_justified(|ui| {
                                let btn = ui.add(layer_btn);

                                if btn.clicked_by(egui::PointerButton::Primary) {
                                    activated_layer = Some((g, l));
                                }
                                if btn.secondary_clicked() {
                                    selected_layers.push((g, l));
                                }
                            });
                        });
                    }
                });
        }
        (activated_layer, selected_layers, selected_groups)
    };

    let scroll_color = Color32::from_rgba_unmultiplied(0, 0, 0, 50);
    let height = ui.available_height() - (ui.style().spacing.item_spacing.y * 3.0); // spacing between the elements
    let calc_paint_rect = |ui: &egui::Ui| -> egui::Rect {
        let available_rect = ui.available_rect_before_wrap();
        let rect = egui::Rect::from_min_size(
            ui.cursor().min,
            egui::vec2(available_rect.width(), available_rect.height()),
        );
        rect
    };

    egui_extras::StripBuilder::new(ui)
        .size(Size::exact(height * 0.35))
        .size(Size::exact(height * 0.3))
        .size(Size::exact(height * 0.35))
        .vertical(|mut strip| {
            // background
            strip.cell(|ui| {
                ui.add(egui::Separator::default().spacing(15.0));
                ui.vertical_centered(|ui| {
                    ui.heading("Background");
                });
                ui.add(egui::Separator::default().spacing(15.0));

                ui.vertical_centered_justified(|ui| {
                    ui.painter()
                        .rect_filled(calc_paint_rect(ui), 0.0, scroll_color);
                    egui::ScrollArea::vertical()
                        .id_source(format!("scroll-bg"))
                        .show(ui, |ui| {
                            let groups_res = group_ui(
                                "bg-groups",
                                ui,
                                &map.resources,
                                &mut map.groups.background,
                            );
                            if let (Some((g, l)), _, _) = groups_res {
                                activated_layer =
                                    Some(EditorMapSetLayer::Background { group: g, layer: l });
                            }
                            selected_layers.extend(&mut groups_res.1.into_iter().map(|(g, l)| {
                                EditorMapSetLayer::Background { group: g, layer: l }
                            }));
                            selected_groups.extend(
                                &mut groups_res
                                    .2
                                    .into_iter()
                                    .map(|g| EditorMapSetGroup::Background { group: g }),
                            );
                        });
                });
            });

            // physics
            strip.cell(|ui| {
                ui.add(egui::Separator::default().spacing(15.0));
                ui.vertical_centered(|ui| {
                    ui.heading("Physics");
                });
                ui.add(egui::Separator::default().spacing(15.0));

                ui.vertical_centered_justified(|ui| {
                    ui.painter()
                        .rect_filled(calc_paint_rect(ui), 0.0, scroll_color);
                    egui::ScrollArea::vertical()
                        .id_source(format!("scroll-phy"))
                        .show(ui, |ui| {
                            let group = &mut map.groups.physics;
                            CollapsingState::load_with_default_open(
                                ui.ctx(),
                                "physics-group".into(),
                                true,
                            )
                            .show_header(ui, |ui| {
                                ui.with_layout(Layout::right_to_left(egui::Align::Min), |ui| {
                                    let hidden = group.editor_attr_mut().hidden;
                                    let hide_btn = Button::new(if hidden {
                                        icon_font_text(ui, "\u{f070}")
                                    } else {
                                        icon_font_text(ui, "\u{f06e}")
                                    })
                                    .selected(group.editor_attr().hidden)
                                    .fill(ui.style().visuals.window_fill);
                                    if ui.add(hide_btn).clicked() {
                                        group.editor_attr_mut().hidden = !hidden;
                                    }
                                    ui.vertical_centered_justified(|ui| {
                                        let btn = Button::new("Physics").frame(false);
                                        if ui.add(btn).secondary_clicked() {
                                            selected_groups.push(EditorMapSetGroup::Physics);
                                        }
                                    })
                                })
                            })
                            .body(|ui| {
                                for (l, layer) in map.groups.physics.layers.iter_mut().enumerate() {
                                    let layer_btn = {
                                        let mut btn = egui::Button::new(layer_name_phy(layer, l));
                                        if layer.editor_attr().active {
                                            btn = btn.selected(true);
                                        }
                                        if layer.user().selected.is_some() {
                                            btn = btn.stroke(button_selected_style());
                                        }
                                        btn
                                    };

                                    ui.with_layout(Layout::right_to_left(egui::Align::Min), |ui| {
                                        let hidden = layer.editor_attr_mut().hidden;
                                        let hide_btn = Button::new(if hidden {
                                            icon_font_text(ui, "\u{f070}")
                                        } else {
                                            icon_font_text(ui, "\u{f06e}")
                                        })
                                        .selected(layer.editor_attr().hidden);
                                        if ui.add(hide_btn).clicked() {
                                            layer.editor_attr_mut().hidden = !hidden;
                                        }

                                        ui.vertical_centered_justified(|ui| {
                                            let btn = ui.add(layer_btn);
                                            if btn.clicked_by(egui::PointerButton::Primary) {
                                                activated_layer =
                                                    Some(EditorMapSetLayer::Physics { layer: l });
                                            }
                                            if btn.secondary_clicked() {
                                                selected_layers
                                                    .push(EditorMapSetLayer::Physics { layer: l });
                                            }
                                        });
                                    });
                                }
                            });
                        });
                });
            });

            // foreground
            strip.cell(|ui| {
                ui.add(egui::Separator::default().spacing(15.0));
                ui.vertical_centered(|ui| {
                    ui.heading("Foreground");
                });
                ui.add(egui::Separator::default().spacing(15.0));

                ui.vertical_centered_justified(|ui| {
                    ui.painter()
                        .rect_filled(calc_paint_rect(ui), 0.0, scroll_color);
                    egui::ScrollArea::vertical()
                        .id_source(format!("scroll-fg"))
                        .show(ui, |ui| {
                            let groups_res = group_ui(
                                "fg-groups",
                                ui,
                                &map.resources,
                                &mut map.groups.foreground,
                            );
                            if let (Some((g, l)), _, _) = groups_res {
                                activated_layer =
                                    Some(EditorMapSetLayer::Foreground { group: g, layer: l });
                            }
                            selected_layers.extend(&mut groups_res.1.into_iter().map(|(g, l)| {
                                EditorMapSetLayer::Foreground { group: g, layer: l }
                            }));
                            selected_groups.extend(
                                &mut groups_res
                                    .2
                                    .into_iter()
                                    .map(|g| EditorMapSetGroup::Foreground { group: g }),
                            );

                            if let Some(activated_layer) = activated_layer {
                                map.set_active_layer(activated_layer);
                            }
                            for selected_layer in selected_layers {
                                map.toggle_selected_layer(selected_layer, false);
                            }
                            for selected_group in selected_groups {
                                map.toggle_selected_group(selected_group, false);
                            }
                        });
                });
            });
        });
}
