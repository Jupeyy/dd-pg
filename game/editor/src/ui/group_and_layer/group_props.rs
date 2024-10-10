use egui::Color32;
use map::{map::groups::layers::tiles::MapTileLayerPhysicsTiles, types::NonZeroU16MinusOne};
use math::math::vector::ffixed;
use ui_base::{types::UiRenderPipe, utils::toggle_ui};

use crate::{
    actions::actions::{
        ActAddRemGroup, ActChangeGroupAttr, ActChangePhysicsGroupAttr, ActRemGroup, EditorAction,
    },
    map::{EditorMapInterface, EditorPhysicsLayer},
    ui::{group_and_layer::shared::copy_tiles, user_data::UserDataWithTab},
};

pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserDataWithTab>, main_frame_only: bool) {
    #[derive(Debug, PartialEq, Eq)]
    enum GroupAttrMode {
        Design,
        Physics,
        /// only design groups selected
        DesignMulti,
        /// design & physics groups mixed
        DesignAndPhysicsMulti,
        None,
    }

    // check which groups are `selected`
    let tab = &mut *pipe.user_data.editor_tab;
    let map = &mut tab.map;
    let window_props = &mut map.user.ui_values.group_attr;

    let bg_selection = map
        .groups
        .background
        .iter()
        .filter(|bg| bg.user.selected.is_some());
    let fg_selection = map
        .groups
        .foreground
        .iter()
        .filter(|fg| fg.user.selected.is_some());
    let bg_selected = bg_selection.count();
    let phy_selected = map.groups.physics.user.selected.is_some();
    let fg_selected = fg_selection.count();

    let mut attr_mode = GroupAttrMode::None;
    if bg_selected > 0 {
        attr_mode = if bg_selected == 1 {
            GroupAttrMode::Design
        } else {
            GroupAttrMode::DesignMulti
        };
    }
    if phy_selected {
        if attr_mode == GroupAttrMode::None {
            attr_mode = GroupAttrMode::Physics;
        } else {
            attr_mode = GroupAttrMode::DesignAndPhysicsMulti;
        }
    }
    if fg_selected > 0 {
        if attr_mode == GroupAttrMode::None {
            attr_mode = if bg_selected == 1 {
                GroupAttrMode::Design
            } else {
                GroupAttrMode::DesignMulti
            };
        } else if attr_mode == GroupAttrMode::Design {
            attr_mode = GroupAttrMode::DesignMulti;
        } else if attr_mode == GroupAttrMode::Physics {
            attr_mode = GroupAttrMode::DesignAndPhysicsMulti;
        }
    }

    let mut bg_selection = map
        .groups
        .background
        .iter_mut()
        .enumerate()
        .filter(|(_, bg)| bg.user.selected.is_some())
        .map(|(g, bg)| (true, g, bg));
    let mut fg_selection = map
        .groups
        .foreground
        .iter_mut()
        .enumerate()
        .filter(|(_, fg)| fg.user.selected.is_some())
        .map(|(g, bg)| (false, g, bg));
    let window_res = match attr_mode {
        GroupAttrMode::Design => {
            let (is_background, g, group) = bg_selection
                .next()
                .unwrap_or_else(|| fg_selection.next().unwrap());

            if main_frame_only {
                ui.painter().rect_filled(
                    window_props.rect,
                    ui.style().visuals.window_rounding,
                    Color32::from_rgba_unmultiplied(0, 0, 0, 255),
                );
                None
            } else {
                let mut window = egui::Window::new("Design Group Attributes")
                    .resizable(false)
                    .collapsible(false);
                window = window.default_rect(window_props.rect);

                let res = window.show(ui.ctx(), |ui| {
                    // render group attributes
                    let group_editor = group.user.selected.as_mut().unwrap();
                    let attr = &mut group_editor.attr;
                    let attr_cmp = *attr;
                    let mut delete_group = false;
                    egui::Grid::new("design group attr grid")
                        .num_columns(2)
                        .spacing([20.0, 4.0])
                        .show(ui, |ui| {
                            // pos x
                            ui.label("Pos x");
                            let mut x = attr.offset.x.to_num::<f64>();
                            ui.add(egui::DragValue::new(&mut x));
                            attr.offset.x = ffixed::from_num(x);
                            ui.end_row();
                            // pos y
                            ui.label("Pos y");
                            let mut y = attr.offset.y.to_num::<f64>();
                            ui.add(egui::DragValue::new(&mut y));
                            attr.offset.y = ffixed::from_num(y);
                            ui.end_row();
                            // para x
                            ui.label("Parallax x");
                            let mut x = attr.parallax.x.to_num::<f64>();
                            ui.add(egui::DragValue::new(&mut x));
                            attr.parallax.x = ffixed::from_num(x);
                            ui.end_row();
                            // para y
                            ui.label("Parallax y");
                            let mut y = attr.parallax.y.to_num::<f64>();
                            ui.add(egui::DragValue::new(&mut y));
                            attr.parallax.y = ffixed::from_num(y);
                            ui.end_row();
                            // clipping on/off
                            ui.label("Clipping");
                            let mut clip_on_off = attr.clipping.is_some();
                            toggle_ui(ui, &mut clip_on_off);
                            ui.end_row();
                            if attr.clipping.is_some() != clip_on_off {
                                if clip_on_off {
                                    attr.clipping = Some(Default::default());
                                } else {
                                    attr.clipping = None;
                                }
                            }
                            if let Some(clipping) = &mut attr.clipping {
                                // clipping x
                                ui.label("Clipping - x");
                                let mut x = clipping.pos.x.to_num::<f64>();
                                ui.add(egui::DragValue::new(&mut x));
                                clipping.pos.x = ffixed::from_num(x);
                                ui.end_row();
                                // clipping y
                                ui.label("Clipping - y");
                                let mut y = clipping.pos.y.to_num::<f64>();
                                ui.add(egui::DragValue::new(&mut y));
                                clipping.pos.y = ffixed::from_num(y);
                                ui.end_row();
                                // clipping w
                                ui.label("Clipping - width");
                                let mut x = clipping.size.x.to_num::<f64>();
                                ui.add(egui::DragValue::new(&mut x));
                                clipping.size.x = ffixed::from_num(x);
                                ui.end_row();
                                // clipping h
                                ui.label("Clipping - height");
                                let mut y = clipping.size.y.to_num::<f64>();
                                ui.add(egui::DragValue::new(&mut y));
                                clipping.size.y = ffixed::from_num(y);
                                ui.end_row();
                            }
                            // name
                            ui.label("Group name");
                            ui.text_edit_singleline(&mut group_editor.name);
                            ui.end_row();
                            // delete
                            if ui.button("Delete group").clicked() {
                                delete_group = true;
                            }
                            ui.end_row();
                        });

                    if *attr != attr_cmp {
                        tab.client.execute(
                            EditorAction::ChangeGroupAttr(ActChangeGroupAttr {
                                is_background,
                                group_index: g,
                                old_attr: group.attr,
                                new_attr: *attr,
                            }),
                            Some(&format!("change-design-group-attr-{is_background}-{g}")),
                        );
                    } else if delete_group {
                        tab.client.execute(
                            EditorAction::RemGroup(ActRemGroup {
                                base: ActAddRemGroup {
                                    is_background,
                                    index: g,
                                    group: group.clone().into(),
                                },
                            }),
                            None,
                        );
                    }
                });
                res
            }
        }
        GroupAttrMode::Physics => {
            if main_frame_only {
                ui.painter().rect_filled(
                    window_props.rect,
                    ui.style().visuals.window_rounding,
                    Color32::from_rgba_unmultiplied(0, 0, 0, 255),
                );
                None
            } else {
                // width & height, nothing else
                let group = &mut map.groups.physics;
                let mut window = egui::Window::new("Physics Group Attributes")
                    .resizable(false)
                    .collapsible(false);
                window = window.default_rect(window_props.rect);
                let res = window.show(ui.ctx(), |ui| {
                    egui::Grid::new("design group attr grid")
                        .num_columns(2)
                        .spacing([20.0, 4.0])
                        .show(ui, |ui| {
                            // render physics group attributes
                            let attr = group.user.selected.as_mut().unwrap();
                            let attr_cmp = attr.clone();
                            // w
                            ui.label("width");
                            let mut w = attr.width.get();
                            ui.add(egui::DragValue::new(&mut w).range(1..=u16::MAX - 1));
                            attr.width = NonZeroU16MinusOne::new(w).unwrap();
                            ui.end_row();
                            // h
                            ui.label("height");
                            let mut h = attr.height.get();
                            ui.add(egui::DragValue::new(&mut h).range(1..=u16::MAX - 1));
                            attr.height = NonZeroU16MinusOne::new(h).unwrap();
                            ui.end_row();
                            if *attr != attr_cmp {
                                let old_layer_tiles: Vec<_> = group
                                    .layers
                                    .iter()
                                    .map(|layer| match layer {
                                        EditorPhysicsLayer::Arbitrary(_) => {
                                            panic!("arbitrary tile layers are unsupported")
                                        }
                                        EditorPhysicsLayer::Game(layer) => {
                                            MapTileLayerPhysicsTiles::Game(
                                                layer.layer.tiles.clone(),
                                            )
                                        }
                                        EditorPhysicsLayer::Front(layer) => {
                                            MapTileLayerPhysicsTiles::Front(
                                                layer.layer.tiles.clone(),
                                            )
                                        }
                                        EditorPhysicsLayer::Tele(layer) => {
                                            MapTileLayerPhysicsTiles::Tele(
                                                layer.layer.base.tiles.clone(),
                                            )
                                        }
                                        EditorPhysicsLayer::Speedup(layer) => {
                                            MapTileLayerPhysicsTiles::Speedup(
                                                layer.layer.tiles.clone(),
                                            )
                                        }
                                        EditorPhysicsLayer::Switch(layer) => {
                                            MapTileLayerPhysicsTiles::Switch(
                                                layer.layer.base.tiles.clone(),
                                            )
                                        }
                                        EditorPhysicsLayer::Tune(layer) => {
                                            MapTileLayerPhysicsTiles::Tune(
                                                layer.layer.base.tiles.clone(),
                                            )
                                        }
                                    })
                                    .collect();
                                tab.client.execute(
                                    EditorAction::ChangePhysicsGroupAttr(
                                        ActChangePhysicsGroupAttr {
                                            old_attr: group.attr.clone(),
                                            new_attr: attr.clone(),

                                            new_layer_tiles: {
                                                let width_or_height_change = group.attr.width
                                                    != attr.width
                                                    || group.attr.height != attr.height;
                                                if width_or_height_change {
                                                    let width_old = group.attr.width.get() as usize;
                                                    let height_old =
                                                        group.attr.height.get() as usize;
                                                    let width_new = attr.width.get() as usize;
                                                    let height_new = attr.height.get() as usize;
                                                    group
                                            .layers
                                            .iter()
                                            .map(|layer| match layer {
                                                EditorPhysicsLayer::Arbitrary(_) => {
                                                    panic!("arbitrary tile layers are unsupported")
                                                }
                                                EditorPhysicsLayer::Game(layer) => {
                                                    MapTileLayerPhysicsTiles::Game(copy_tiles(
                                                        width_old, height_old, width_new,
                                                        height_new, &layer.layer.tiles,
                                                    ))
                                                }
                                                EditorPhysicsLayer::Front(layer) => {
                                                    MapTileLayerPhysicsTiles::Front(copy_tiles(
                                                        width_old, height_old, width_new,
                                                        height_new, &layer.layer.tiles,
                                                    ))
                                                }
                                                EditorPhysicsLayer::Tele(layer) => {
                                                    MapTileLayerPhysicsTiles::Tele(copy_tiles(
                                                        width_old, height_old, width_new,
                                                        height_new, &layer.layer.base.tiles,
                                                    ))
                                                }
                                                EditorPhysicsLayer::Speedup(layer) => {
                                                    MapTileLayerPhysicsTiles::Speedup(copy_tiles(
                                                        width_old, height_old, width_new,
                                                        height_new, &layer.layer.tiles,
                                                    ))
                                                }
                                                EditorPhysicsLayer::Switch(layer) => {
                                                    MapTileLayerPhysicsTiles::Switch(copy_tiles(
                                                        width_old, height_old, width_new,
                                                        height_new, &layer.layer.base.tiles,
                                                    ))
                                                }
                                                EditorPhysicsLayer::Tune(layer) => {
                                                    MapTileLayerPhysicsTiles::Tune(copy_tiles(
                                                        width_old, height_old, width_new,
                                                        height_new, &layer.layer.base.tiles,
                                                    ))
                                                }
                                            })
                                            .collect()
                                                } else {
                                                    old_layer_tiles.clone()
                                                }
                                            },
                                            old_layer_tiles,
                                        },
                                    ),
                                    Some("change-physics-group-attr"),
                                );
                            }
                        });
                });
                res
            }
        }
        GroupAttrMode::DesignMulti => todo!(),
        GroupAttrMode::DesignAndPhysicsMulti => todo!(),
        GroupAttrMode::None => {
            // render nothing
            None
        }
    };

    if window_res.is_some() && !main_frame_only {
        let window_res = window_res.as_ref().unwrap();
        window_props.rect = window_res.response.rect;
    }

    *pipe.user_data.pointer_is_used |= if let Some(window_res) = &window_res {
        let intersected = ui.input(|i| {
            if i.pointer.primary_down() {
                Some((
                    !window_res.response.rect.intersects({
                        let min = i.pointer.interact_pos().unwrap_or_default();
                        let max = min;
                        [min, max].into()
                    }),
                    i.pointer.primary_pressed(),
                ))
            } else {
                None
            }
        });
        if intersected.is_some_and(|(outside, clicked)| outside && clicked) {
            map.unselect_all(true, true);
        }
        intersected.is_some_and(|(outside, _)| !outside)
    } else {
        false
    };
}
