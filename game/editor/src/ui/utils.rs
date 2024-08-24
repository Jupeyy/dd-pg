use egui::{text::LayoutJob, FontId, TextFormat};
use map::skeleton::groups::layers::design::MapLayerSkeleton;

use crate::map::{EditorGroup, EditorLayer, EditorPhysicsLayer, EditorResources};

pub fn group_name(group: &EditorGroup, index: usize) -> String {
    if group.name.is_empty() {
        format!("Group #{}", index)
    } else {
        format!("Group \"{}\"", group.name)
    }
}

pub fn layer_name(
    ui: &egui::Ui,
    resources: &EditorResources,
    layer: &EditorLayer,
    index: usize,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    match layer {
        MapLayerSkeleton::Abritrary(_) => todo!(),
        MapLayerSkeleton::Tile(_) => append_icon_font_text(&mut job, ui, "\u{f00a}"),
        MapLayerSkeleton::Quad(_) => append_icon_font_text(&mut job, ui, "\u{f61f}"),
        MapLayerSkeleton::Sound(_) => append_icon_font_text(&mut job, ui, "\u{f001}"),
    };
    if !layer.name().is_empty() {
        job.append(
            &format!(" Layer \"{}\"", layer.name()),
            0.0,
            TextFormat {
                font_id: FontId::new(12.0, egui::FontFamily::default()),
                color: ui.style().visuals.text_color(),
                valign: egui::Align::Center,
                ..Default::default()
            },
        );
    } else if let Some((icon, text)) = match layer {
        MapLayerSkeleton::Abritrary(_) => {
            todo!()
        }
        MapLayerSkeleton::Tile(layer) => {
            layer.layer.attr.image_array.map(|image| (
                    "\u{f302}",
                    format!(" {}", resources.image_arrays[image].def.name.clone()),
                ))
        }
        MapLayerSkeleton::Quad(layer) => {
            layer.layer.attr.image.map(|image| (
                    "\u{f03e}",
                    format!(" {}", resources.images[image].def.name.clone()),
                ))
        }
        MapLayerSkeleton::Sound(layer) => {
            layer.layer.attr.sound.map(|sound| (
                    "\u{f001}",
                    format!(" {}", resources.sounds[sound].def.name.clone()),
                ))
        }
    } {
        job.append(
            " Layer \"",
            0.0,
            TextFormat {
                font_id: FontId::new(12.0, egui::FontFamily::default()),
                color: ui.style().visuals.text_color(),
                valign: egui::Align::Center,
                ..Default::default()
            },
        );
        append_icon_font_text(&mut job, ui, icon);
        job.append(
            &format!("{}\"", text),
            0.0,
            TextFormat {
                font_id: FontId::new(12.0, egui::FontFamily::default()),
                color: ui.style().visuals.text_color(),
                valign: egui::Align::Center,
                ..Default::default()
            },
        );
    } else {
        job.append(
            &format!(" Layer #{}", index),
            0.0,
            TextFormat {
                font_id: FontId::new(12.0, egui::FontFamily::default()),
                color: ui.style().visuals.text_color(),
                valign: egui::Align::Center,
                ..Default::default()
            },
        );
    }
    job
}

pub fn layer_name_phy(layer: &EditorPhysicsLayer, index: usize) -> String {
    let layer_name = match layer {
        EditorPhysicsLayer::Arbitrary(_) => {
            todo!()
        }
        EditorPhysicsLayer::Game(_) => "Game",
        EditorPhysicsLayer::Front(_) => "Front",
        EditorPhysicsLayer::Tele(_) => "Tele",
        EditorPhysicsLayer::Speedup(_) => "Speedup",
        EditorPhysicsLayer::Switch(_) => "Switch",
        EditorPhysicsLayer::Tune(_) => "Tune",
    };
    format!("#{} {layer_name}", index)
}

pub fn append_icon_font_text(job: &mut LayoutJob, ui: &egui::Ui, text: &str) {
    job.append(
        text,
        0.0,
        TextFormat {
            font_id: FontId::new(12.0, egui::FontFamily::Name("icons".into())),
            color: ui.style().visuals.text_color(),
            valign: egui::Align::Center,
            ..Default::default()
        },
    );
}

pub fn icon_font_text(ui: &egui::Ui, text: &str) -> LayoutJob {
    let mut job = LayoutJob::default();
    append_icon_font_text(&mut job, ui, text);
    job
}
