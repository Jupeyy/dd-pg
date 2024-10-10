use crate::actions::actions::{
    ActAddGroup, ActAddPhysicsTileLayer, ActAddQuadLayer, ActAddRemGroup,
    ActAddRemPhysicsTileLayer, ActAddRemQuadLayer, ActAddRemSoundLayer, ActAddRemTileLayer,
    ActAddSoundLayer, ActAddTileLayer, EditorAction,
};
use crate::client::EditorClient;
use crate::map::EditorPhysicsLayer;
use crate::ui::user_data::UserDataWithTab;
use crate::{
    map::{
        EditorCommonLayerOrGroupAttrInterface, EditorDesignLayerInterface, EditorGroup,
        EditorMapInterface, EditorMapSetGroup, EditorMapSetLayer, EditorResources,
    },
    ui::utils::{group_name, icon_font_text, layer_name, layer_name_phy},
};

use egui::{collapsing_header::CollapsingState, Button, Color32, Layout};
use egui_extras::{Size, StripBuilder};
use map::map::groups::layers::design::{
    MapLayerQuad, MapLayerQuadsAttrs, MapLayerSound, MapLayerSoundAttrs, MapLayerTile,
};
use map::map::groups::layers::physics::{
    MapLayerPhysics, MapLayerTilePhysicsBase, MapLayerTilePhysicsSwitch, MapLayerTilePhysicsTele,
    MapLayerTilePhysicsTune,
};
use map::map::groups::layers::tiles::MapTileLayerAttr;
use map::map::groups::MapGroup;
use map::types::NonZeroU16MinusOne;
use math::math::vector::{nffixed, nfvec4};
use ui_base::types::UiRenderPipe;

fn button_selected_style() -> egui::Stroke {
    egui::Stroke::new(2.0, Color32::LIGHT_GREEN)
}

pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserDataWithTab>) {
    let tab = &mut *pipe.user_data.editor_tab;
    let map = &mut tab.map;

    let mut activated_layer = None;
    let mut selected_layers = Vec::new();
    let mut selected_groups = Vec::new();
    let group_ui = |id: &str,
                    ui: &mut egui::Ui,
                    resources: &EditorResources,
                    groups: &mut Vec<EditorGroup>,
                    is_background: bool,
                    client: &mut EditorClient| {
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
                            let mut btn = egui::Button::new(layer_name(ui, resources, layer, l));
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

                                if btn.clicked() {
                                    activated_layer = Some((g, l));
                                }
                                if btn.secondary_clicked() {
                                    selected_layers.push((g, l));
                                }
                            });
                        });
                    }

                    let mut job = icon_font_text(ui, "\u{f0fe}");
                    job.append("Add design layer", 5.0, egui::TextFormat::default());
                    ui.menu_button(job, |ui| {
                        if ui.button("Tile").clicked() {
                            let add_layer = MapLayerTile {
                                attr: MapTileLayerAttr {
                                    width: NonZeroU16MinusOne::new(50).unwrap(),
                                    height: NonZeroU16MinusOne::new(50).unwrap(),
                                    color: nfvec4::new(
                                        nffixed::const_from_int(1),
                                        nffixed::const_from_int(1),
                                        nffixed::const_from_int(1),
                                        nffixed::const_from_int(1),
                                    ),
                                    high_detail: false,
                                    color_anim: None,
                                    color_anim_offset: time::Duration::ZERO,
                                    image_array: None,
                                },
                                tiles: vec![Default::default(); 50 * 50],
                                name: "".into(),
                            };
                            client.execute(
                                EditorAction::AddTileLayer(ActAddTileLayer {
                                    base: ActAddRemTileLayer {
                                        is_background,
                                        group_index: g,
                                        index: group.layers.len(),
                                        layer: add_layer,
                                    },
                                }),
                                None,
                            );
                        }
                        if ui.button("Quad").clicked() {
                            let add_layer = MapLayerQuad {
                                attr: MapLayerQuadsAttrs {
                                    image: None,
                                    high_detail: false,
                                },
                                quads: vec![],
                                name: "".into(),
                            };
                            client.execute(
                                EditorAction::AddQuadLayer(ActAddQuadLayer {
                                    base: ActAddRemQuadLayer {
                                        is_background,
                                        group_index: g,
                                        index: group.layers.len(),
                                        layer: add_layer,
                                    },
                                }),
                                None,
                            );
                        }
                        if ui.button("Sound").clicked() {
                            let add_layer = MapLayerSound {
                                attr: MapLayerSoundAttrs {
                                    sound: None,
                                    high_detail: false,
                                },
                                sounds: vec![],
                                name: "".into(),
                            };
                            client.execute(
                                EditorAction::AddSoundLayer(ActAddSoundLayer {
                                    base: ActAddRemSoundLayer {
                                        is_background,
                                        group_index: g,
                                        index: group.layers.len(),
                                        layer: add_layer,
                                    },
                                }),
                                None,
                            );
                        }
                    });
                });
        }
        (activated_layer, selected_layers, selected_groups)
    };

    let scroll_color = Color32::from_rgba_unmultiplied(0, 0, 0, 50);
    let height = ui.available_height() - (ui.style().spacing.item_spacing.y * 3.0); // spacing between the elements
    let calc_paint_rect = |ui: &egui::Ui| -> egui::Rect {
        let available_rect = ui.available_rect_before_wrap();

        egui::Rect::from_min_size(
            ui.cursor().min,
            egui::vec2(available_rect.width(), available_rect.height()),
        )
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
                    StripBuilder::new(ui)
                        .size(Size::remainder())
                        .size(Size::exact(20.0))
                        .vertical(|mut strip| {
                            strip.cell(|ui| {
                                egui::ScrollArea::vertical()
                                    .id_salt("scroll-bg".to_string())
                                    .show(ui, |ui| {
                                        let groups_res = group_ui(
                                            "bg-groups",
                                            ui,
                                            &map.resources,
                                            &mut map.groups.background,
                                            true,
                                            &mut tab.client,
                                        );
                                        if let (Some((g, l)), _, _) = groups_res {
                                            activated_layer = Some(EditorMapSetLayer::Background {
                                                group: g,
                                                layer: l,
                                            });
                                        }
                                        selected_layers.extend(&mut groups_res.1.into_iter().map(
                                            |(g, l)| EditorMapSetLayer::Background {
                                                group: g,
                                                layer: l,
                                            },
                                        ));
                                        selected_groups.extend(
                                            &mut groups_res.2.into_iter().map(|g| {
                                                EditorMapSetGroup::Background { group: g }
                                            }),
                                        );
                                    });
                            });
                            strip.cell(|ui| {
                                let mut job = icon_font_text(ui, "\u{f0fe}");
                                job.append("Add design group", 5.0, egui::TextFormat::default());
                                if ui.button(job).clicked() {
                                    tab.client.execute(
                                        EditorAction::AddGroup(ActAddGroup {
                                            base: ActAddRemGroup {
                                                is_background: true,
                                                index: map.groups.background.len(),
                                                group: MapGroup {
                                                    attr: Default::default(),
                                                    layers: Default::default(),
                                                    name: "".into(),
                                                },
                                            },
                                        }),
                                        None,
                                    );
                                }
                            });
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
                    StripBuilder::new(ui)
                        .size(Size::remainder())
                        .size(Size::exact(20.0))
                        .vertical(|mut strip| {
                            strip.cell(|ui| {
                                egui::ScrollArea::vertical()
                                    .id_salt("scroll-phy".to_string())
                                    .show(ui, |ui| {
                                        let group = &mut map.groups.physics;
                                        CollapsingState::load_with_default_open(
                                            ui.ctx(),
                                            "physics-group".into(),
                                            true,
                                        )
                                        .show_header(ui, |ui| {
                                            ui.with_layout(
                                                Layout::right_to_left(egui::Align::Min),
                                                |ui| {
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
                                                        let btn =
                                                            Button::new("Physics").frame(false);
                                                        if ui.add(btn).secondary_clicked() {
                                                            selected_groups
                                                                .push(EditorMapSetGroup::Physics);
                                                        }
                                                    })
                                                },
                                            )
                                        })
                                        .body(|ui| {
                                            for (l, layer) in
                                                map.groups.physics.layers.iter_mut().enumerate()
                                            {
                                                let layer_btn = {
                                                    let mut btn =
                                                        egui::Button::new(layer_name_phy(layer, l));
                                                    if layer.editor_attr().active {
                                                        btn = btn.selected(true);
                                                    }
                                                    if layer.user().selected.is_some() {
                                                        btn = btn.stroke(button_selected_style());
                                                    }
                                                    btn
                                                };

                                                ui.with_layout(
                                                    Layout::right_to_left(egui::Align::Min),
                                                    |ui| {
                                                        let hidden = layer.editor_attr_mut().hidden;
                                                        let hide_btn = Button::new(if hidden {
                                                            icon_font_text(ui, "\u{f070}")
                                                        } else {
                                                            icon_font_text(ui, "\u{f06e}")
                                                        })
                                                        .selected(layer.editor_attr().hidden);
                                                        if ui.add(hide_btn).clicked() {
                                                            layer.editor_attr_mut().hidden =
                                                                !hidden;
                                                        }

                                                        ui.vertical_centered_justified(|ui| {
                                                            let btn = ui.add(layer_btn);
                                                            if btn.clicked() {
                                                                activated_layer = Some(
                                                                    EditorMapSetLayer::Physics {
                                                                        layer: l,
                                                                    },
                                                                );
                                                            }
                                                            if btn.secondary_clicked() {
                                                                selected_layers.push(
                                                                    EditorMapSetLayer::Physics {
                                                                        layer: l,
                                                                    },
                                                                );
                                                            }
                                                        });
                                                    },
                                                );
                                            }
                                        });
                                    });
                            });

                            strip.cell(|ui| {
                                #[derive(Debug, Default)]
                                struct FoundPhyLayers {
                                    front: bool,
                                    tele: bool,
                                    speedup: bool,
                                    switch: bool,
                                    tune: bool,
                                }
                                let mut phy_layers = FoundPhyLayers::default();

                                let physics = &map.groups.physics;
                                physics.layers.iter().for_each(|layer| {
                                    match layer {
                                        EditorPhysicsLayer::Arbitrary(_)
                                        | EditorPhysicsLayer::Game(_) => {
                                            // ignore
                                        }
                                        EditorPhysicsLayer::Front(_) => phy_layers.front = true,
                                        EditorPhysicsLayer::Tele(_) => phy_layers.tele = true,
                                        EditorPhysicsLayer::Speedup(_) => phy_layers.speedup = true,
                                        EditorPhysicsLayer::Switch(_) => phy_layers.switch = true,
                                        EditorPhysicsLayer::Tune(_) => phy_layers.tune = true,
                                    }
                                });

                                if !phy_layers.front
                                    || !phy_layers.tele
                                    || !phy_layers.speedup
                                    || !phy_layers.switch
                                    || !phy_layers.tune
                                {
                                    ui.add_space(10.0);
                                    let mut job = icon_font_text(ui, "\u{f0fe}");
                                    job.append(
                                        "Add physics layer",
                                        5.0,
                                        egui::TextFormat::default(),
                                    );
                                    ui.menu_button(job, |ui| {
                                        let mut add_layer = None;
                                        if !phy_layers.front && ui.button("Front").clicked() {
                                            add_layer = Some(MapLayerPhysics::Front(
                                                MapLayerTilePhysicsBase {
                                                    tiles: vec![
                                                        Default::default();
                                                        physics.attr.width.get() as usize
                                                            * physics.attr.height.get()
                                                                as usize
                                                    ],
                                                },
                                            ));
                                        }
                                        if !phy_layers.tele && ui.button("Tele").clicked() {
                                            add_layer = Some(MapLayerPhysics::Tele(
                                                MapLayerTilePhysicsTele {
                                                    base: MapLayerTilePhysicsBase {
                                                        tiles: vec![
                                                            Default::default();
                                                            physics.attr.width.get()
                                                                as usize
                                                                * physics.attr.height.get()
                                                                    as usize
                                                        ],
                                                    },
                                                    tele_names: Default::default(),
                                                },
                                            ));
                                        }
                                        if !phy_layers.switch && ui.button("Switch").clicked() {
                                            add_layer = Some(MapLayerPhysics::Switch(
                                                MapLayerTilePhysicsSwitch {
                                                    base: MapLayerTilePhysicsBase {
                                                        tiles: vec![
                                                            Default::default();
                                                            physics.attr.width.get()
                                                                as usize
                                                                * physics.attr.height.get()
                                                                    as usize
                                                        ],
                                                    },
                                                    switch_names: Default::default(),
                                                },
                                            ));
                                        }
                                        if !phy_layers.speedup && ui.button("Speedup").clicked() {
                                            add_layer = Some(MapLayerPhysics::Speedup(
                                                MapLayerTilePhysicsBase {
                                                    tiles: vec![
                                                        Default::default();
                                                        physics.attr.width.get() as usize
                                                            * physics.attr.height.get()
                                                                as usize
                                                    ],
                                                },
                                            ));
                                        }
                                        if !phy_layers.tune && ui.button("Tune").clicked() {
                                            add_layer = Some(MapLayerPhysics::Tune(
                                                MapLayerTilePhysicsTune {
                                                    base: MapLayerTilePhysicsBase {
                                                        tiles: vec![
                                                            Default::default();
                                                            physics.attr.width.get()
                                                                as usize
                                                                * physics.attr.height.get()
                                                                    as usize
                                                        ],
                                                    },
                                                    tune_zones: Default::default(),
                                                },
                                            ));
                                        }

                                        if let Some(add_layer) = add_layer {
                                            tab.client.execute(
                                                EditorAction::AddPhysicsTileLayer(
                                                    ActAddPhysicsTileLayer {
                                                        base: ActAddRemPhysicsTileLayer {
                                                            index: physics.layers.len(),
                                                            layer: add_layer,
                                                        },
                                                    },
                                                ),
                                                None,
                                            );
                                        }
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
                    StripBuilder::new(ui)
                        .size(Size::remainder())
                        .size(Size::exact(20.0))
                        .vertical(|mut strip| {
                            strip.cell(|ui| {
                                egui::ScrollArea::vertical()
                                    .id_salt("scroll-fg".to_string())
                                    .show(ui, |ui| {
                                        let groups_res = group_ui(
                                            "fg-groups",
                                            ui,
                                            &map.resources,
                                            &mut map.groups.foreground,
                                            false,
                                            &mut tab.client,
                                        );
                                        if let (Some((g, l)), _, _) = groups_res {
                                            activated_layer = Some(EditorMapSetLayer::Foreground {
                                                group: g,
                                                layer: l,
                                            });
                                        }
                                        selected_layers.extend(&mut groups_res.1.into_iter().map(
                                            |(g, l)| EditorMapSetLayer::Foreground {
                                                group: g,
                                                layer: l,
                                            },
                                        ));
                                        selected_groups.extend(
                                            &mut groups_res.2.into_iter().map(|g| {
                                                EditorMapSetGroup::Foreground { group: g }
                                            }),
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
                            strip.cell(|ui| {
                                let mut job = icon_font_text(ui, "\u{f0fe}");
                                job.append("Add design group", 5.0, egui::TextFormat::default());
                                if ui.button(job).clicked() {
                                    tab.client.execute(
                                        EditorAction::AddGroup(ActAddGroup {
                                            base: ActAddRemGroup {
                                                is_background: false,
                                                index: map.groups.foreground.len(),
                                                group: MapGroup {
                                                    attr: Default::default(),
                                                    layers: Default::default(),
                                                    name: "".into(),
                                                },
                                            },
                                        }),
                                        None,
                                    );
                                }
                            });
                        });
                });
            });
        });
}
