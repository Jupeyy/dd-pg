use egui::Button;
use ui_base::types::{UIPipe, UIState};

use crate::{
    explain::{TEXT_QUAD_SELECTION, TEXT_TILE_BRUSH},
    tools::tool::{ActiveTool, ActiveToolQuads, ActiveToolSounds, ActiveToolTiles},
    ui::{user_data::UserDataWithTab, utils::icon_font_text},
};

pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserDataWithTab>,
    ui_state: &mut UIState,
    main_frame_only: bool,
) {
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
                                        egui_commonmark::CommonMarkViewer::new(
                                            "tile-brush-tooltip",
                                        )
                                        .show(
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
                                        egui_commonmark::CommonMarkViewer::new(
                                            "quad-selection-tooltip",
                                        )
                                        .show(
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
}
