use egui::Button;
use ui_base::types::UiRenderPipe;

use crate::{
    explain::{TEXT_QUAD_SELECTION, TEXT_TILE_BRUSH, TEXT_TILE_BRUSH_MIRROR},
    map::EditorMapInterface,
    tools::tool::{ActiveTool, ActiveToolQuads, ActiveToolSounds, ActiveToolTiles},
    ui::{user_data::UserDataWithTab, utils::icon_font_text},
};

use super::tile_mirror::{
    mirror_layer_tiles_x, mirror_layer_tiles_y, mirror_tiles_x, mirror_tiles_y,
    rotate_layer_tiles_plus_90, rotate_tile_flags_plus_90, rotate_tiles_plus_90,
};

pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserDataWithTab>, main_frame_only: bool) {
    let style = ui.style();
    let height = style.spacing.interact_size.y + style.spacing.item_spacing.y;
    egui::TopBottomPanel::top("top_toolbar")
        .resizable(false)
        .default_height(height)
        .height_range(height..=height)
        .show_inside(ui, |ui| {
            egui::ScrollArea::horizontal().show(ui, |ui| {
                if main_frame_only {
                } else {
                    ui.horizontal(|ui| {
                        match &mut pipe.user_data.tools.active_tool {
                            ActiveTool::Tiles(tool) => {
                                // brush
                                let mut btn = Button::new(icon_font_text(ui, "\u{f55d}"));
                                if matches!(tool, ActiveToolTiles::Brush) {
                                    btn = btn.selected(true);
                                }
                                if ui
                                    .add(btn)
                                    .on_hover_ui(|ui| {
                                        let mut cache = egui_commonmark::CommonMarkCache::default();
                                        egui_commonmark::CommonMarkViewer::new().show(
                                            ui,
                                            &mut cache,
                                            TEXT_TILE_BRUSH,
                                        );
                                    })
                                    .clicked()
                                {
                                    *tool = ActiveToolTiles::Brush;
                                }
                                // select
                                let mut btn = Button::new(icon_font_text(ui, "\u{f247}"));
                                if matches!(tool, ActiveToolTiles::Selection) {
                                    btn = btn.selected(true);
                                }
                                if ui.add(btn).clicked() {
                                    *tool = ActiveToolTiles::Selection;
                                }
                            }
                            ActiveTool::Quads(tool) => {
                                // brush
                                let mut btn = Button::new(icon_font_text(ui, "\u{f55d}"));
                                if matches!(tool, ActiveToolQuads::Brush) {
                                    btn = btn.selected(true);
                                }
                                if ui.add(btn).clicked() {
                                    *tool = ActiveToolQuads::Brush;
                                }
                                // select
                                let mut btn = Button::new(icon_font_text(ui, "\u{f45c}"));
                                if matches!(tool, ActiveToolQuads::Selection) {
                                    btn = btn.selected(true);
                                }
                                if ui
                                    .add(btn)
                                    .on_hover_ui(|ui| {
                                        let mut cache = egui_commonmark::CommonMarkCache::default();
                                        egui_commonmark::CommonMarkViewer::new().show(
                                            ui,
                                            &mut cache,
                                            TEXT_QUAD_SELECTION,
                                        );
                                    })
                                    .clicked()
                                {
                                    *tool = ActiveToolQuads::Selection;
                                }
                            }
                            ActiveTool::Sounds(tool) => {
                                // brush
                                let mut btn = Button::new(icon_font_text(ui, "\u{f55d}"));
                                if matches!(tool, ActiveToolSounds::Brush) {
                                    btn = btn.selected(true);
                                }
                                if ui.add(btn).clicked() {
                                    *tool = ActiveToolSounds::Brush;
                                }
                            }
                        }
                    });
                }
            });
        });

    let tools = &mut pipe.user_data.tools;
    match &tools.active_tool {
        ActiveTool::Tiles(tool) => {
            let is_active = (matches!(tool, ActiveToolTiles::Brush)
                && tools.tiles.brush.brush.is_some())
                || (matches!(tool, ActiveToolTiles::Selection)
                    && tools.tiles.selection.range.is_some());
            egui::TopBottomPanel::top("top_toolbar_tiles_extra")
                .resizable(false)
                .default_height(height)
                .height_range(height..=height)
                .show_inside(ui, |ui| {
                    egui::ScrollArea::horizontal().show(ui, |ui| {
                        ui.add_enabled_ui(is_active, |ui| {
                            if main_frame_only {
                            } else {
                                ui.horizontal(|ui| {
                                    // mirror y
                                    let btn = Button::new(icon_font_text(ui, "\u{f07d}"));
                                    if ui
                                        .add(btn)
                                        .on_hover_ui(|ui| {
                                            let mut cache =
                                                egui_commonmark::CommonMarkCache::default();
                                            egui_commonmark::CommonMarkViewer::new().show(
                                                ui,
                                                &mut cache,
                                                TEXT_TILE_BRUSH_MIRROR,
                                            );
                                        })
                                        .clicked()
                                    {
                                        match tool {
                                            ActiveToolTiles::Brush => {
                                                if let Some(brush) = &mut tools.tiles.brush.brush {
                                                    mirror_tiles_y(
                                                        pipe.user_data.tp,
                                                        pipe.user_data.graphics_mt,
                                                        pipe.user_data.buffer_object_handle,
                                                        pipe.user_data.backend_handle,
                                                        brush,
                                                        true,
                                                    );
                                                }
                                            }
                                            ActiveToolTiles::Selection => {
                                                if let (Some(layer), Some(range)) = (
                                                    pipe.user_data.editor_tab.map.active_layer(),
                                                    &tools.tiles.selection.range,
                                                ) {
                                                    mirror_layer_tiles_y(
                                                        pipe.user_data.tp,
                                                        layer,
                                                        range,
                                                        &mut pipe.user_data.editor_tab.client,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                    // mirror x
                                    let btn = Button::new(icon_font_text(ui, "\u{f07e}"));
                                    if ui.add(btn).clicked() {
                                        match tool {
                                            ActiveToolTiles::Brush => {
                                                if let Some(brush) = &mut tools.tiles.brush.brush {
                                                    mirror_tiles_x(
                                                        pipe.user_data.tp,
                                                        pipe.user_data.graphics_mt,
                                                        pipe.user_data.buffer_object_handle,
                                                        pipe.user_data.backend_handle,
                                                        brush,
                                                        true,
                                                    );
                                                }
                                            }
                                            ActiveToolTiles::Selection => {
                                                if let (Some(layer), Some(range)) = (
                                                    pipe.user_data.editor_tab.map.active_layer(),
                                                    &tools.tiles.selection.range,
                                                ) {
                                                    mirror_layer_tiles_x(
                                                        pipe.user_data.tp,
                                                        layer,
                                                        range,
                                                        &mut pipe.user_data.editor_tab.client,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                    match tool {
                                        ActiveToolTiles::Brush => {
                                            // rotate -90°
                                            let btn = Button::new(icon_font_text(ui, "\u{f2ea}"));
                                            if ui.add(btn).clicked() {
                                                if let Some(brush) = &mut tools.tiles.brush.brush {
                                                    // use 3 times 90° here, bcs the 90° logic also "fixes" the cursor
                                                    // x,y mirror does not
                                                    rotate_tiles_plus_90(
                                                        pipe.user_data.tp,
                                                        pipe.user_data.graphics_mt,
                                                        pipe.user_data.buffer_object_handle,
                                                        pipe.user_data.backend_handle,
                                                        brush,
                                                        false,
                                                    );
                                                    rotate_tiles_plus_90(
                                                        pipe.user_data.tp,
                                                        pipe.user_data.graphics_mt,
                                                        pipe.user_data.buffer_object_handle,
                                                        pipe.user_data.backend_handle,
                                                        brush,
                                                        false,
                                                    );
                                                    rotate_tiles_plus_90(
                                                        pipe.user_data.tp,
                                                        pipe.user_data.graphics_mt,
                                                        pipe.user_data.buffer_object_handle,
                                                        pipe.user_data.backend_handle,
                                                        brush,
                                                        true,
                                                    );
                                                }
                                            }
                                            // rotate +90°
                                            let btn = Button::new(icon_font_text(ui, "\u{f2f9}"));
                                            if ui.add(btn).clicked() {
                                                if let Some(brush) = &mut tools.tiles.brush.brush {
                                                    rotate_tiles_plus_90(
                                                        pipe.user_data.tp,
                                                        pipe.user_data.graphics_mt,
                                                        pipe.user_data.buffer_object_handle,
                                                        pipe.user_data.backend_handle,
                                                        brush,
                                                        true,
                                                    );
                                                }
                                            }
                                            // rotate tiles (only by flags) +90°
                                            let btn = Button::new(icon_font_text(ui, "\u{e4f6}"));
                                            if ui.add(btn).clicked() {
                                                if let Some(brush) = &mut tools.tiles.brush.brush {
                                                    rotate_tile_flags_plus_90(
                                                        pipe.user_data.tp,
                                                        pipe.user_data.graphics_mt,
                                                        pipe.user_data.buffer_object_handle,
                                                        pipe.user_data.backend_handle,
                                                        brush,
                                                        true,
                                                    );
                                                }
                                            }
                                        }
                                        ActiveToolTiles::Selection => {
                                            if let Some(layer) =
                                                pipe.user_data.editor_tab.map.active_layer()
                                            {
                                                // rotate inner tiles (flags) by 90°
                                                let btn =
                                                    Button::new(icon_font_text(ui, "\u{e4f6}"));
                                                if ui.add(btn).clicked() {
                                                    if let Some(range) =
                                                        &tools.tiles.selection.range
                                                    {
                                                        rotate_layer_tiles_plus_90(
                                                            pipe.user_data.tp,
                                                            layer,
                                                            range,
                                                            &mut pipe.user_data.editor_tab.client,
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                });
                            }
                        });
                    });
                });
        }
        ActiveTool::Quads(_) => {
            egui::TopBottomPanel::top("top_toolbar_quads_extra")
                .resizable(false)
                .default_height(height)
                .height_range(height..=height)
                .show_inside(ui, |ui| {
                    egui::ScrollArea::horizontal().show(ui, |ui| {
                        if main_frame_only {
                        } else {
                            ui.horizontal(|ui| {
                                // add quad
                                let btn = Button::new(icon_font_text(ui, "\u{f0fe}"));
                                if ui
                                    .add(btn)
                                    .on_hover_ui(|ui| {
                                        let mut cache = egui_commonmark::CommonMarkCache::default();
                                        egui_commonmark::CommonMarkViewer::new().show(
                                            ui,
                                            &mut cache,
                                            TEXT_TILE_BRUSH,
                                        );
                                    })
                                    .clicked()
                                {}
                            });
                        }
                    });
                });
        }
        ActiveTool::Sounds(_) => {
            egui::TopBottomPanel::top("top_toolbar_sound_extra")
                .resizable(false)
                .default_height(height)
                .height_range(height..=height)
                .show_inside(ui, |ui| {
                    egui::ScrollArea::horizontal().show(ui, |ui| {
                        if main_frame_only {
                        } else {
                            ui.horizontal(|ui| {
                                // add sound
                                let btn = Button::new(icon_font_text(ui, "\u{f0fe}"));
                                if ui
                                    .add(btn)
                                    .on_hover_ui(|ui| {
                                        let mut cache = egui_commonmark::CommonMarkCache::default();
                                        egui_commonmark::CommonMarkViewer::new().show(
                                            ui,
                                            &mut cache,
                                            TEXT_TILE_BRUSH,
                                        );
                                    })
                                    .clicked()
                                {}
                            });
                        }
                    });
                });
        }
    }

    super::tune::render(ui, pipe, main_frame_only);
    super::switch::render(ui, pipe, main_frame_only);
    super::tele::render(ui, pipe, main_frame_only);
}
