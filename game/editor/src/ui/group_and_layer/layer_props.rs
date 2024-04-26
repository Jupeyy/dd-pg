use egui::{text::LayoutJob, Color32, InnerResponse, TextFormat};
use map::types::NonZeroU16MinusOne;
use math::math::vector::{nffixed, nfvec4};
use ui_base::{
    types::{UIPipe, UIState},
    utils::toggle_ui,
};

use crate::{
    actions::actions::{
        ActChangeQuadLayerAttr, ActChangeSoundLayerAttr, ActChangeTileLayerDesignAttr, EditorAction,
    },
    explain::TEXT_LAYER_PROPS_COLOR,
    map::{EditorDesignLayerInterface, EditorLayer, EditorMapInterface, ResourceSelection},
    ui::{
        group_and_layer::{
            resource_selector::ResourceSelectionMode,
            shared::{animations_panel_open_warning, copy_tiles},
        },
        user_data::UserDataWithTab,
        utils::append_icon_font_text,
    },
};

pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserDataWithTab>,
    ui_state: &mut UIState,
    main_frame_only: bool,
) {
    #[derive(Debug, PartialEq, Eq)]
    enum LayerAttrMode {
        DesignTile,
        DesignQuad,
        DesignSound,
        /// only tile layers selected
        DesignTileMulti,
        /// only quad layers selected
        DesignQuadMulti,
        /// only sound layers selected
        DesignSoundMulti,
        /// all design layers mixed, only `high detail` is shared across all
        DesignMulti,
        /// empty attr
        Physics,
        /// mixing physics & design always leads to empty attr intersection
        PhysicsDesignMulti,
        None,
    }

    // check which layers are `selected`
    let tab = &mut *pipe.user_data.editor_tab;
    let map = &mut tab.map;
    let animations_panel_open =
        map.user.ui_values.animations_panel_open && !map.user.options.no_animations_with_properties;
    let bg_selection = map
        .groups
        .background
        .iter()
        .flat_map(|bg| bg.layers.iter().filter(|layer| layer.is_selected()));
    let fg_selection = map
        .groups
        .foreground
        .iter()
        .flat_map(|fg| fg.layers.iter().filter(|layer| layer.is_selected()));
    let phy_selection = map
        .groups
        .physics
        .layers
        .iter()
        .filter(|layer| layer.user().selected.is_some());

    let bg_selected = bg_selection.clone().count();
    let phy_selected = phy_selection.clone().count();
    let fg_selected = fg_selection.clone().count();

    let mut attr_mode = LayerAttrMode::None;
    if bg_selected > 0 {
        let tile_count = bg_selection
            .clone()
            .filter(|layer| matches!(layer, EditorLayer::Tile(_)))
            .count();
        let quad_count = bg_selection
            .clone()
            .filter(|layer| matches!(layer, EditorLayer::Quad(_)))
            .count();
        let sound_count = bg_selection
            .clone()
            .filter(|layer| matches!(layer, EditorLayer::Sound(_)))
            .count();
        if tile_count > 0 {
            attr_mode = if tile_count == 1 {
                LayerAttrMode::DesignTile
            } else {
                LayerAttrMode::DesignTileMulti
            };
        }
        if quad_count > 0 {
            if attr_mode == LayerAttrMode::None {
                attr_mode = if quad_count == 1 {
                    LayerAttrMode::DesignQuad
                } else {
                    LayerAttrMode::DesignQuadMulti
                };
            } else {
                attr_mode = LayerAttrMode::DesignMulti;
            }
        }
        if sound_count > 0 {
            if attr_mode == LayerAttrMode::None {
                attr_mode = if sound_count == 1 {
                    LayerAttrMode::DesignSound
                } else {
                    LayerAttrMode::DesignSoundMulti
                };
            } else {
                attr_mode = LayerAttrMode::DesignMulti;
            }
        }
    }
    if phy_selected > 0 {
        if attr_mode == LayerAttrMode::None {
            // ignore multi here, bcs phy attr are always empty
            attr_mode = LayerAttrMode::Physics;
        } else {
            attr_mode = LayerAttrMode::PhysicsDesignMulti;
        }
    }
    if fg_selected > 0 {
        let tile_count = fg_selection
            .clone()
            .filter(|layer| matches!(layer, EditorLayer::Tile(_)))
            .count();
        let quad_count = fg_selection
            .clone()
            .filter(|layer| matches!(layer, EditorLayer::Quad(_)))
            .count();
        let sound_count = fg_selection
            .clone()
            .filter(|layer| matches!(layer, EditorLayer::Sound(_)))
            .count();
        if tile_count > 0 {
            if attr_mode == LayerAttrMode::None {
                attr_mode = if tile_count == 1 {
                    LayerAttrMode::DesignTile
                } else {
                    LayerAttrMode::DesignTileMulti
                };
            } else if let LayerAttrMode::Physics | LayerAttrMode::PhysicsDesignMulti = attr_mode {
                attr_mode = LayerAttrMode::PhysicsDesignMulti;
            } else {
                attr_mode = LayerAttrMode::DesignMulti;
            }
        }
        if quad_count > 0 {
            if attr_mode == LayerAttrMode::None {
                attr_mode = if quad_count == 1 {
                    LayerAttrMode::DesignQuad
                } else {
                    LayerAttrMode::DesignQuadMulti
                };
            } else if let LayerAttrMode::Physics | LayerAttrMode::PhysicsDesignMulti = attr_mode {
                attr_mode = LayerAttrMode::PhysicsDesignMulti;
            } else {
                attr_mode = LayerAttrMode::DesignMulti;
            }
        }
        if sound_count > 0 {
            if attr_mode == LayerAttrMode::None {
                attr_mode = if sound_count == 1 {
                    LayerAttrMode::DesignSound
                } else {
                    LayerAttrMode::DesignSoundMulti
                };
            } else if let LayerAttrMode::Physics | LayerAttrMode::PhysicsDesignMulti = attr_mode {
                attr_mode = LayerAttrMode::PhysicsDesignMulti;
            } else {
                attr_mode = LayerAttrMode::DesignMulti;
            }
        }
    }

    let mut bg_selection = map
        .groups
        .background
        .iter_mut()
        .enumerate()
        .flat_map(|(g, bg)| {
            bg.layers
                .iter_mut()
                .enumerate()
                .filter(|(_, layer)| layer.is_selected())
                .map(move |l| (true, g, l))
        });
    let mut fg_selection = map
        .groups
        .foreground
        .iter_mut()
        .enumerate()
        .flat_map(|(g, fg)| {
            fg.layers
                .iter_mut()
                .enumerate()
                .filter(|(_, layer)| layer.is_selected())
                .map(move |l| (false, g, l))
        });
    let phy_selection = map
        .groups
        .physics
        .layers
        .iter_mut()
        .enumerate()
        .filter(|(_, layer)| layer.user().selected.is_some());

    let window_props = &mut map.user.ui_values.layer_attr;

    let mut resource_selector_was_outside = true;
    let window_res = match attr_mode {
        LayerAttrMode::DesignTile => {
            let (is_background, g, (l, EditorLayer::Tile(layer))) = bg_selection
                .next()
                .unwrap_or_else(|| fg_selection.next().unwrap())
            else {
                panic!("not a tile layer, bug in above calculations")
            };
            let layer_editor = layer.user.selected.as_mut().unwrap();
            let layer_attr_cmp = layer_editor.attr.clone();

            let mut window = egui::Window::new("Design Tile Layer Attributes")
                .resizable(false)
                .collapsible(false);
            if main_frame_only {
                window = window.fixed_rect(window_props.rect);
            } else {
                window = window.default_rect(window_props.rect);
            }

            let res = window.show(ui.ctx(), |ui| {
                egui::Grid::new("design group attr grid")
                    .num_columns(2)
                    .spacing([20.0, 4.0])
                    .show(ui, |ui| {
                        let attr = &mut layer_editor.attr;
                        // detail
                        ui.label("High detail");
                        toggle_ui(ui, &mut attr.high_detail);
                        ui.end_row();
                        // w
                        ui.label("Width");
                        let mut w = attr.width.get();
                        ui.add(egui::DragValue::new(&mut w).clamp_range(1..=u16::MAX - 1));
                        attr.width = NonZeroU16MinusOne::new(w).unwrap();
                        ui.end_row();
                        // h
                        ui.label("Height");
                        let mut h = attr.height.get();
                        ui.add(egui::DragValue::new(&mut h).clamp_range(1..=u16::MAX - 1));
                        attr.height = NonZeroU16MinusOne::new(h).unwrap();
                        ui.end_row();
                        // image
                        if ui
                            .add(
                                egui::Button::new("Image selection")
                                    .selected(layer_editor.image_2d_array_selection_open.is_some()),
                            )
                            .clicked()
                        {
                            layer_editor.image_2d_array_selection_open = layer_editor
                                .image_2d_array_selection_open
                                .is_none()
                                .then_some(ResourceSelection {
                                    hovered_resource: None,
                                });
                        }
                        ui.end_row();
                        // color
                        let mut job = LayoutJob::default();
                        job.append(
                            "Color ",
                            0.0,
                            TextFormat {
                                color: ui.style().visuals.text_color(),
                                valign: egui::Align::Center,
                                ..Default::default()
                            },
                        );
                        append_icon_font_text(&mut job, ui, "\u{f05a}");
                        ui.label(job).on_hover_ui(|ui| {
                            let mut cache = egui_commonmark::CommonMarkCache::default();
                            egui_commonmark::CommonMarkViewer::new("layer-props-color-tooltip")
                                .show(ui, &mut cache, TEXT_LAYER_PROPS_COLOR);
                        });
                        let mut color = [
                            attr.color.r().to_num::<f32>(),
                            attr.color.g().to_num::<f32>(),
                            attr.color.b().to_num::<f32>(),
                            attr.color.a().to_num::<f32>(),
                        ];
                        ui.color_edit_button_rgba_unmultiplied(&mut color);
                        attr.color = nfvec4::new(
                            nffixed::from_num(color[0]),
                            nffixed::from_num(color[1]),
                            nffixed::from_num(color[2]),
                            nffixed::from_num(color[3]),
                        );
                        ui.end_row();
                        // color anim
                        ui.label("TODO: color anim");
                        ui.end_row();
                        // color time offset
                        ui.label("TODO: color anim time offset");
                        ui.end_row();
                        // name
                        ui.label("Name");
                        ui.text_edit_singleline(&mut layer_editor.name);
                        ui.end_row();

                        if animations_panel_open {
                            ui.colored_label(
                                Color32::RED,
                                "The animation panel is open,\n\
                                    changing attributes will not apply\n\
                                    them to the quad permanently!",
                            )
                            .on_hover_ui(animations_panel_open_warning);
                        }
                        ui.end_row();
                    })
            });

            if let Some(resource_selection) = &mut layer_editor.image_2d_array_selection_open {
                resource_selection.hovered_resource = None;
                let res = super::resource_selector::render(
                    ui,
                    pipe.user_data.pointer_is_used,
                    &map.resources.image_arrays,
                    &mut map.user.ui_values.resource_selector,
                    ui_state,
                    main_frame_only,
                );
                resource_selector_was_outside = res.pointer_was_outside;
                if let Some(resource) = res.mode {
                    match resource {
                        ResourceSelectionMode::Hovered(index) => {
                            resource_selection.hovered_resource = Some(index);
                        }
                        ResourceSelectionMode::Clicked(index) => {
                            layer_editor.attr.image_array = index;
                        }
                    }
                }
                if res.pointer_was_outside {
                    layer_editor.image_2d_array_selection_open = None;
                }
            }

            if layer_editor.attr != layer_attr_cmp && !animations_panel_open {
                tab.client.execute(
                    EditorAction::ChangeTileLayerDesignAttr(ActChangeTileLayerDesignAttr {
                        is_background,
                        group_index: g,
                        layer_index: l,
                        old_attr: layer.layer.attr.clone(),
                        new_attr: layer_editor.attr.clone(),

                        old_tiles: layer.layer.tiles.clone(),
                        new_tiles: {
                            let width_or_height_change = layer.layer.attr.width
                                != layer_editor.attr.width
                                || layer.layer.attr.height != layer_editor.attr.height;
                            if width_or_height_change {
                                let width_old = layer.layer.attr.width.get() as usize;
                                let height_old = layer.layer.attr.height.get() as usize;
                                let width_new = layer_editor.attr.width.get() as usize;
                                let height_new = layer_editor.attr.height.get() as usize;
                                copy_tiles(
                                    width_old,
                                    height_old,
                                    width_new,
                                    height_new,
                                    &layer.layer.tiles,
                                )
                            } else {
                                layer.layer.tiles.clone()
                            }
                        },
                    }),
                    Some(&format!(
                        "change-design-tile-layer-attr-{is_background}-{g}-{l}"
                    )),
                );
            }

            if res.is_some() && !main_frame_only {
                let res = res.as_ref().unwrap();
                window_props.rect = res.response.rect;
            }

            res
        }
        LayerAttrMode::DesignQuad => {
            let (is_background, g, (l, EditorLayer::Quad(layer))) = bg_selection
                .next()
                .unwrap_or_else(|| fg_selection.next().unwrap())
            else {
                panic!("not a quad layer, bug in above calculations")
            };
            let layer_editor = layer.user.selected.as_mut().unwrap();
            let layer_attr_cmp = layer_editor.attr.clone();

            let mut window = egui::Window::new("Design Quad Layer Attributes")
                .resizable(false)
                .collapsible(false);
            if main_frame_only {
                window = window.fixed_rect(window_props.rect);
            } else {
                window = window.default_rect(window_props.rect);
            }

            let res = window.show(ui.ctx(), |ui| {
                egui::Grid::new("design group attr grid")
                    .num_columns(2)
                    .spacing([20.0, 4.0])
                    .show(ui, |ui| {
                        let attr = &mut layer_editor.attr;
                        // detail
                        ui.label("High detail");
                        toggle_ui(ui, &mut attr.high_detail);
                        ui.end_row();
                        // image
                        if ui
                            .add(
                                egui::Button::new("Image selection")
                                    .selected(layer_editor.image_selection_open.is_some()),
                            )
                            .clicked()
                        {
                            layer_editor.image_selection_open = layer_editor
                                .image_selection_open
                                .is_none()
                                .then_some(ResourceSelection {
                                    hovered_resource: None,
                                });
                        }
                        ui.end_row();
                        // name
                        ui.label("Name");
                        ui.text_edit_singleline(&mut layer_editor.name);
                        ui.end_row();
                    })
            });

            if let Some(resource_selection) = &mut layer_editor.image_selection_open {
                resource_selection.hovered_resource = None;
                let res = super::resource_selector::render(
                    ui,
                    pipe.user_data.pointer_is_used,
                    &map.resources.images,
                    &mut map.user.ui_values.resource_selector,
                    ui_state,
                    main_frame_only,
                );
                resource_selector_was_outside = res.pointer_was_outside;
                if let Some(resource) = res.mode {
                    match resource {
                        ResourceSelectionMode::Hovered(index) => {
                            resource_selection.hovered_resource = Some(index);
                        }
                        ResourceSelectionMode::Clicked(index) => {
                            layer_editor.attr.image = index;
                        }
                    }
                }
                if res.pointer_was_outside {
                    layer_editor.image_selection_open = None;
                }
            }

            if layer_editor.attr != layer_attr_cmp {
                tab.client.execute(
                    EditorAction::ChangeQuadLayerAttr(ActChangeQuadLayerAttr {
                        is_background,
                        group_index: g,
                        layer_index: l,
                        old_attr: layer.layer.attr.clone(),
                        new_attr: layer_editor.attr.clone(),
                    }),
                    Some(&format!("change-quad-layer-attr-{is_background}-{g}-{l}")),
                );
            }

            if res.is_some() && !main_frame_only {
                window_props.rect = res.as_ref().unwrap().response.rect;
            }

            res
        }
        LayerAttrMode::DesignSound => {
            let (is_background, g, (l, EditorLayer::Sound(layer))) = bg_selection
                .next()
                .unwrap_or_else(|| fg_selection.next().unwrap())
            else {
                panic!("not a sound layer, bug in above calculations")
            };
            let layer_editor = layer.user.selected.as_mut().unwrap();
            let layer_attr_cmp = layer_editor.attr.clone();

            let mut window = egui::Window::new("Design Sound Layer Attributes")
                .resizable(false)
                .collapsible(false);
            if main_frame_only {
                window = window.fixed_rect(window_props.rect);
            } else {
                window = window.default_rect(window_props.rect);
            }

            let res = window.show(ui.ctx(), |ui| {
                egui::Grid::new("design group attr grid")
                    .num_columns(2)
                    .spacing([20.0, 4.0])
                    .show(ui, |ui| {
                        let attr = &mut layer_editor.attr;
                        // detail
                        ui.label("High detail");
                        toggle_ui(ui, &mut attr.high_detail);
                        ui.end_row();
                        // sound
                        if ui
                            .add(
                                egui::Button::new("Sound selection")
                                    .selected(layer_editor.sound_selection_open.is_some()),
                            )
                            .clicked()
                        {
                            layer_editor.sound_selection_open = layer_editor
                                .sound_selection_open
                                .is_none()
                                .then_some(ResourceSelection {
                                    hovered_resource: None,
                                });
                        }
                        ui.end_row();
                        // name
                        ui.label("Name");
                        ui.text_edit_singleline(&mut layer_editor.name);
                        ui.end_row();
                    })
            });

            if let Some(resource_selection) = &mut layer_editor.sound_selection_open {
                resource_selection.hovered_resource = None;
                let res = super::resource_selector::render(
                    ui,
                    pipe.user_data.pointer_is_used,
                    &map.resources.sounds,
                    &mut map.user.ui_values.resource_selector,
                    ui_state,
                    main_frame_only,
                );
                resource_selector_was_outside = res.pointer_was_outside;
                if let Some(resource) = res.mode {
                    match resource {
                        ResourceSelectionMode::Hovered(index) => {
                            resource_selection.hovered_resource = Some(index);
                        }
                        ResourceSelectionMode::Clicked(index) => {
                            layer_editor.attr.sound = index;
                        }
                    }
                }
                if res.pointer_was_outside {
                    layer_editor.sound_selection_open = None;
                }
            }

            if layer_editor.attr != layer_attr_cmp {
                tab.client.execute(
                    EditorAction::ChangeSoundLayerAttr(ActChangeSoundLayerAttr {
                        is_background,
                        group_index: g,
                        layer_index: l,
                        old_attr: layer.layer.attr.clone(),
                        new_attr: layer_editor.attr.clone(),
                    }),
                    Some(&format!("change-sound-layer-attr-{is_background}-{g}-{l}")),
                );
            }

            if res.is_some() && !main_frame_only {
                window_props.rect = res.as_ref().unwrap().response.rect;
            }

            res
        }
        LayerAttrMode::DesignTileMulti => todo!(),
        LayerAttrMode::DesignQuadMulti => todo!(),
        LayerAttrMode::DesignSoundMulti => todo!(),
        LayerAttrMode::DesignMulti => todo!(),
        LayerAttrMode::Physics => {
            let mut window = egui::Window::new("Physics Layer Attributes")
                .resizable(false)
                .collapsible(false);

            if main_frame_only {
                window = window.fixed_rect(window_props.rect);
            } else {
                window = window.default_rect(window_props.rect);
            }

            let res = window.show(ui.ctx(), |ui| {
                ui.label("Physics layers have no properties. Look in the physics group instead.")
            });

            if res.is_some() && !main_frame_only {
                window_props.rect = res.as_ref().unwrap().response.rect;
            }

            res.map(|res| {
                InnerResponse::new(
                    res.inner.map(|res| InnerResponse::new((), res)),
                    res.response,
                )
            })
        }
        LayerAttrMode::PhysicsDesignMulti => todo!(),
        LayerAttrMode::None => {
            // render nothing
            None
        }
    };
    *pipe.user_data.pointer_is_used |= if let Some(window_res) = window_res {
        let intersected = ui.input(|i| {
            if i.pointer.primary_down() {
                Some((
                    !window_res.response.rect.intersects({
                        let min = i.pointer.interact_pos().unwrap_or_default().into();
                        let max = min;
                        [min, max].into()
                    }),
                    i.pointer.primary_pressed(),
                ))
            } else {
                None
            }
        });
        if intersected
            .is_some_and(|(outside, clicked)| outside && clicked && resource_selector_was_outside)
        {
            map.unselect_all(true, true);
        }
        intersected.is_some_and(|(outside, _)| !outside)
    } else {
        false
    }
}
