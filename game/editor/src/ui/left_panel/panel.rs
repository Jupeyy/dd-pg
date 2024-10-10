use egui::{Button, Color32};
use ui_base::types::UiRenderPipe;

use crate::{
    explain::{
        TEXT_2D_IMAGE_ARRAY, TEXT_IMAGES, TEXT_LAYERS_AND_GROUPS_OVERVIEW, TEXT_SOUND_SOURCES,
    },
    map::{EditorGroupPanelTab, EditorMapPropsUiWindow},
    ui::{user_data::UserDataWithTab, utils::icon_font_text},
};

pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserDataWithTab>, main_frame_only: bool) {
    let tab = &mut *pipe.user_data.editor_tab;
    let map = &mut tab.map;

    let res =
        if main_frame_only {
            ui.painter().rect_filled(
                map.user.ui_values.groups_panel.rect,
                ui.style().visuals.window_rounding,
                Color32::from_rgba_unmultiplied(0, 0, 0, 255),
            );
            None
        } else {
            let mut panel = egui::SidePanel::left("left_panel")
                .resizable(true)
                .width_range(120.0..=260.0);
            panel = panel.default_width(map.user.ui_values.groups_panel.rect.width());

            Some(panel.show_inside(ui, |ui| {
                let map = &mut pipe.user_data.editor_tab.map;
                let panel_tab = &mut map.user.ui_values.group_panel_active_tab;
                ui.vertical_centered_justified(|ui| {
                    ui.with_layout(
                        egui::Layout::from_main_dir_and_cross_align(
                            egui::Direction::LeftToRight,
                            egui::Align::Min,
                        )
                        .with_main_align(egui::Align::Center),
                        |ui| {
                            if ui
                                .add(Button::new(icon_font_text(ui, "\u{f5fd}")).selected(
                                    matches!(panel_tab, EditorGroupPanelTab::GroupsAndLayers),
                                ))
                                .on_hover_ui(|ui| {
                                    let mut cache = egui_commonmark::CommonMarkCache::default();
                                    egui_commonmark::CommonMarkViewer::new().show(
                                        ui,
                                        &mut cache,
                                        TEXT_LAYERS_AND_GROUPS_OVERVIEW,
                                    );
                                })
                                .clicked()
                            {
                                *panel_tab = EditorGroupPanelTab::GroupsAndLayers;
                            }
                            if ui
                                .add(
                                    Button::new(icon_font_text(ui, "\u{f03e}")).selected(matches!(
                                        panel_tab,
                                        EditorGroupPanelTab::Images(_)
                                    )),
                                )
                                .on_hover_ui(|ui| {
                                    let mut cache = egui_commonmark::CommonMarkCache::default();
                                    egui_commonmark::CommonMarkViewer::new().show(
                                        ui,
                                        &mut cache,
                                        TEXT_IMAGES,
                                    );
                                })
                                .clicked()
                            {
                                *panel_tab = EditorGroupPanelTab::Images(Default::default());
                            }
                            if ui
                                .add(Button::new(icon_font_text(ui, "\u{f302}")).selected(
                                    matches!(panel_tab, EditorGroupPanelTab::ArrayImages(_)),
                                ))
                                .on_hover_ui(|ui| {
                                    let mut cache = egui_commonmark::CommonMarkCache::default();
                                    egui_commonmark::CommonMarkViewer::new().show(
                                        ui,
                                        &mut cache,
                                        TEXT_2D_IMAGE_ARRAY,
                                    );
                                })
                                .clicked()
                            {
                                *panel_tab = EditorGroupPanelTab::ArrayImages(Default::default());
                            }
                            if ui
                                .add(
                                    Button::new(icon_font_text(ui, "\u{f001}")).selected(matches!(
                                        panel_tab,
                                        EditorGroupPanelTab::Sounds(_)
                                    )),
                                )
                                .on_hover_ui(|ui| {
                                    let mut cache = egui_commonmark::CommonMarkCache::default();
                                    egui_commonmark::CommonMarkViewer::new().show(
                                        ui,
                                        &mut cache,
                                        TEXT_SOUND_SOURCES,
                                    );
                                })
                                .clicked()
                            {
                                *panel_tab = EditorGroupPanelTab::Sounds(Default::default());
                            }
                        },
                    );
                });

                match panel_tab {
                    EditorGroupPanelTab::GroupsAndLayers => {
                        super::groups_and_layers::render(ui, pipe);
                    }
                    EditorGroupPanelTab::Images(panel_data) => {
                        super::images::render(
                            ui,
                            main_frame_only,
                            &mut pipe.user_data.editor_tab.client,
                            &map.groups,
                            &mut map.resources,
                            panel_data,
                            pipe.user_data.io,
                        );
                    }
                    EditorGroupPanelTab::ArrayImages(panel_data) => {
                        super::image_arrays::render(
                            ui,
                            main_frame_only,
                            &mut pipe.user_data.editor_tab.client,
                            &map.groups,
                            &mut map.resources,
                            panel_data,
                            pipe.user_data.io,
                        );
                    }
                    EditorGroupPanelTab::Sounds(panel_data) => {
                        super::sounds::render(
                            ui,
                            main_frame_only,
                            &mut pipe.user_data.editor_tab.client,
                            &map.groups,
                            &mut map.resources,
                            panel_data,
                            pipe.user_data.io,
                        );
                    }
                }
            }))
        };

    let tab = &mut *pipe.user_data.editor_tab;
    let map = &mut tab.map;
    if let Some(res) = res {
        if !main_frame_only {
            map.user.ui_values.groups_panel = EditorMapPropsUiWindow {
                rect: res.response.rect,
            };
        }
    }
}
