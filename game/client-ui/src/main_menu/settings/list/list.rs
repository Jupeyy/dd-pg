use client_containers::container::ContainerItemIndexType;
use egui::{epaint::RectShape, Color32, Layout, ScrollArea, Shape};
use egui_extras::{Size, StripBuilder};
use fuzzy_matcher::FuzzyMatcher;
use math::math::vector::vec2;
use ui_base::{
    components::clearable_edit_field::clearable_edit_field,
    utils::{add_horizontal_margins, icon_font_text_for_text},
};

pub fn render<'a>(
    ui: &mut egui::Ui,
    entries: impl Iterator<Item = (&'a str, ContainerItemIndexType)>,
    entry_visual_size: f32,
    validation_fn: impl Fn(usize, &str) -> anyhow::Result<()>,
    is_selected_fn: impl Fn(usize, &str) -> bool,
    mut render_fn: impl FnMut(&mut egui::Ui, usize, &str, vec2, f32),
    mut on_click_fn: impl FnMut(usize, &str),
    search: &mut String,
    right_from_search: impl FnOnce(&mut egui::Ui),
) {
    ui.style_mut().spacing.scroll.floating = false;
    let search_str = search.clone();
    StripBuilder::new(ui)
        .size(Size::remainder())
        .size(Size::exact(30.0))
        .vertical(|mut strip| {
            strip.cell(|ui| {
                ui.painter().add(Shape::Rect(RectShape::filled(
                    ui.available_rect_before_wrap(),
                    0.0,
                    Color32::from_rgba_unmultiplied(0, 0, 0, 100),
                )));
                ScrollArea::vertical().show(ui, |ui| {
                    add_horizontal_margins(ui, |ui| {
                        ui.with_layout(
                            Layout::left_to_right(egui::Align::Min)
                                .with_main_wrap(true)
                                .with_main_align(egui::Align::Min),
                            |ui| {
                                for (entry_index, (entry_name, ty)) in
                                    entries.enumerate().filter(|(_, (name, _))| {
                                        let matcher = fuzzy_matcher::skim::SkimMatcherV2::default();
                                        matcher.fuzzy_match(name, &search_str).is_some()
                                    })
                                {
                                    super::entry::render(
                                        ui,
                                        entry_index,
                                        entry_name,
                                        ty,
                                        entry_visual_size,
                                        &validation_fn,
                                        &is_selected_fn,
                                        &mut render_fn,
                                        &mut on_click_fn,
                                    );
                                }
                            },
                        );
                    });
                });
            });

            strip.cell(|ui| {
                let width = ui.available_width();
                StripBuilder::new(ui)
                    .size(Size::remainder().at_most(width / 2.0))
                    .size(Size::remainder())
                    .horizontal(|mut strip| {
                        strip.cell(|ui| {
                            ui.horizontal_centered(|ui| {
                                // Search
                                ui.label(icon_font_text_for_text(ui, "\u{f002}"));
                                clearable_edit_field(ui, search, Some(200.0), None);
                            });
                        });
                        strip.cell(|ui| {
                            ui.with_layout(
                                Layout::right_to_left(egui::Align::Center),
                                right_from_search,
                            );
                        })
                    });
            })
        });
}
