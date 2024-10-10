use std::ops::Add;

use egui::{vec2, Color32, Image, Sense, Window};
use egui_file_dialog::{DialogMode, DialogState};
use math::math::vector::vec2_base;
use ui_base::types::UiRenderPipe;

use crate::{
    tools::tile_layer::auto_mapper::TileLayerAutoMapperRun,
    ui::{user_data::UserData, utils::icon_font_text},
};

pub fn render(main_frame_only: bool, pipe: &mut UiRenderPipe<UserData>, ui: &mut egui::Ui) {
    let auto_mapper = &mut *pipe.user_data.auto_mapper;
    auto_mapper.update();

    let window_res = Window::new("Auto-mapper-rule-creator").show(ui.ctx(), |ui| {
        let min_tile_size = 20.0;
        ui.set_min_width(min_tile_size * 9.0 + min_tile_size * 16.0 + 50.0);
        ui.set_min_height(min_tile_size * 9.0 + min_tile_size * 16.0 + 50.0);
        ui.horizontal(|ui| {
            egui::ComboBox::new("auto-mapper-run-selector", "Select rule to edit")
                .selected_text(match auto_mapper.active_rule {
                    Some(rule) => auto_mapper
                        .rules
                        .get(rule)
                        .map(|rule| rule.name.as_str())
                        .unwrap_or("rule not found."),
                    None => "None...",
                })
                .show_ui(ui, |ui| {
                    ui.vertical(|ui| {
                        for (r, rule) in auto_mapper.rules.iter().enumerate() {
                            if ui.add(egui::Button::new(&rule.name)).clicked() {
                                auto_mapper.active_rule = Some(r);
                            }
                        }
                    })
                });

            if ui.button(icon_font_text(ui, "\u{f07c}")).clicked() {
                auto_mapper.file_dialog.select_file();
            }
            if !main_frame_only && auto_mapper.file_dialog.state() == DialogState::Open {
                let mode = auto_mapper.file_dialog.mode();
                if let Some(selected) = auto_mapper
                    .file_dialog
                    .update(ui.ctx())
                    .selected()
                    .map(|path| path.to_path_buf())
                {
                    match mode {
                        DialogMode::SelectFile => {
                            // add rule to loading tasks
                            auto_mapper.load(selected.as_ref(), ui.ctx().clone());
                        }
                        _ => panic!("this was not implemented."),
                    }
                }
            }
        });

        // render rule
        if auto_mapper
            .active_rule
            .is_some_and(|i| auto_mapper.rules.get(i).is_some())
        {
            let rule = auto_mapper
                .rules
                .get_mut(auto_mapper.active_rule.unwrap())
                .unwrap();
            ui.horizontal(|ui| {
                // prev run
                if ui.button(icon_font_text(ui, "\u{f060}")).clicked() {
                    rule.active_run = rule.active_run.saturating_sub(1);
                }

                ui.label(format!("{}", rule.active_run));

                // next run
                if ui.button(icon_font_text(ui, "\u{f061}")).clicked() {
                    rule.active_run = rule.active_run.add(1).clamp(0, rule.runs.len() - 1);
                }

                ui.add_space(10.0);

                // new run
                if ui.button(icon_font_text(ui, "\u{f0fe}")).clicked() {
                    rule.runs.push(TileLayerAutoMapperRun {
                        tiles: Default::default(),
                    });
                }
                // remove cur run
                if ui.button(icon_font_text(ui, "\u{f2ed}")).clicked() && rule.runs.len() > 1 {
                    rule.runs.remove(rule.active_run);
                    rule.active_run = rule.active_run.saturating_sub(1);
                }
            });

            // render current run
            let available_rect = ui.available_rect_before_wrap();
            let size = available_rect.height().min(available_rect.width()) / 2.0 - 10.0;

            let tile_size = size / 16.0;

            let spacing = ui.spacing_mut();
            spacing.item_spacing = vec2(0.0, 0.0);
            spacing.interact_size = vec2(0.0, 0.0);
            ui.painter().rect_filled(
                egui::Rect::from_min_size(
                    available_rect.min,
                    egui::vec2(tile_size * 9.0, tile_size * 9.0),
                ),
                0.0,
                Color32::BLACK,
            );
            ui.add_space(10.0);
            for y in 0..9 {
                if ui
                    .allocate_exact_size((tile_size, tile_size).into(), Sense::click())
                    .1
                    .clicked()
                {
                    if let Some(tile) = auto_mapper.selected_tile {
                        dbg!(tile);
                    } else {
                        auto_mapper.selected_grid = Some(vec2_base::new(y % 3, y / 3));
                    }
                }
            }
            let available_rect = ui.available_rect_before_wrap();
            ui.painter().rect_filled(
                egui::Rect::from_min_size(
                    available_rect.min,
                    egui::vec2(tile_size * 16.0, tile_size * 16.0),
                ),
                0.0,
                Color32::BLACK,
            );
            ui.vertical(|ui| {
                for y in 0..16 {
                    ui.horizontal(|ui| {
                        for x in 0..16 {
                            let tile_index = y * 16 + x;
                            let tile_texture = &rule.user.tile_textures_pngs[tile_index];

                            let mut img =
                                Image::new((tile_texture.id(), vec2(tile_size, tile_size)))
                                    .sense(Sense::click());

                            if auto_mapper
                                .selected_tile
                                .is_some_and(|i| i as usize == tile_index)
                            {
                                img = img.bg_fill(Color32::RED);
                            }

                            if ui.add(img).clicked() {
                                auto_mapper.selected_tile = Some(tile_index as u8);
                            }
                        }
                    });
                }
            });
        } else {
            ui.label("Select a rule to continue...");
        }
    });

    if let Some(window_res) = &window_res {
        auto_mapper.window_rect = window_res.response.rect;
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
        intersected.is_some_and(|(outside, _)| !outside)
    } else {
        false
    };
}
