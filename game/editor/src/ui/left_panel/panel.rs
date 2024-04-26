use egui::Button;
use ui_base::types::{UIPipe, UIState};

use crate::{
    explain::{
        TEXT_2D_IMAGE_ARRAY, TEXT_IMAGES, TEXT_LAYERS_AND_GROUPS_OVERVIEW, TEXT_SOUND_SOURCES,
    },
    map::EditorGroupPanelTab,
    ui::{user_data::UserDataWithTab, utils::icon_font_text},
};

pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserDataWithTab>,
    ui_state: &mut UIState,
    main_frame_only: bool,
) {
    let tab = &mut *pipe.user_data.editor_tab;
    let map = &mut tab.map;

    let mut panel = egui::SidePanel::left("left_panel")
        .resizable(true)
        .width_range(120.0..=260.0);

    if main_frame_only {
        panel = panel.exact_width(map.user.ui_values.groups_panel_width);
        // ^ workaround, always seems to be around 15 pixels too wide
        panel = panel.width_range(
            map.user.ui_values.groups_panel_width - 15.0..=map.user.ui_values.groups_panel_width,
        );
        panel = panel.resizable(false);
    } else {
        panel = panel.default_width(map.user.ui_values.groups_panel_width);
    }

    let res =
        panel.show_inside(ui, |ui| {
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
                            .add(
                                Button::new(icon_font_text(ui, "\u{f5fd}")).selected(matches!(
                                    panel_tab,
                                    EditorGroupPanelTab::GroupsAndLayers
                                )),
                            )
                            .on_hover_ui(|ui| {
                                let mut cache = egui_commonmark::CommonMarkCache::default();
                                egui_commonmark::CommonMarkViewer::new(
                                    "layers-and-groups-overview-tooltip",
                                )
                                .show(
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
                                Button::new(icon_font_text(ui, "\u{f03e}"))
                                    .selected(matches!(panel_tab, EditorGroupPanelTab::Images(_))),
                            )
                            .on_hover_ui(|ui| {
                                let mut cache = egui_commonmark::CommonMarkCache::default();
                                egui_commonmark::CommonMarkViewer::new("images-tooltip").show(
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
                            .add(
                                Button::new(icon_font_text(ui, "\u{f302}")).selected(matches!(
                                    panel_tab,
                                    EditorGroupPanelTab::ArrayImages(_)
                                )),
                            )
                            .on_hover_ui(|ui| {
                                let mut cache = egui_commonmark::CommonMarkCache::default();
                                egui_commonmark::CommonMarkViewer::new("image-2d-arrays-tooltip")
                                    .show(ui, &mut cache, TEXT_2D_IMAGE_ARRAY);
                            })
                            .clicked()
                        {
                            *panel_tab = EditorGroupPanelTab::ArrayImages(Default::default());
                        }
                        if ui
                            .add(
                                Button::new(icon_font_text(ui, "\u{f001}"))
                                    .selected(matches!(panel_tab, EditorGroupPanelTab::Sounds(_))),
                            )
                            .on_hover_ui(|ui| {
                                let mut cache = egui_commonmark::CommonMarkCache::default();
                                egui_commonmark::CommonMarkViewer::new("sound-sources-tooltips")
                                    .show(ui, &mut cache, TEXT_SOUND_SOURCES);
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
                    super::groups_and_layers::render(ui, pipe, ui_state, main_frame_only);
                }
                EditorGroupPanelTab::Images(panel_data) => {
                    super::images::render(
                        ui,
                        ui_state,
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
                        ui_state,
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
                        ui_state,
                        main_frame_only,
                        &mut pipe.user_data.editor_tab.client,
                        &map.groups,
                        &mut map.resources,
                        panel_data,
                        pipe.user_data.io,
                    );
                }
            }
        });

    let tab = &mut *pipe.user_data.editor_tab;
    let map = &mut tab.map;
    if !main_frame_only && res.response.rect.width() != map.user.ui_values.groups_panel_width {
        map.user.ui_values.groups_panel_width = res.response.rect.width();
    }
}
