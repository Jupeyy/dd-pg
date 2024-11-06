use map::skeleton::groups::layers::design::MapLayerSkeleton;

use crate::map::{EditorGroup, EditorLayer, EditorPhysicsLayer, EditorResources};

pub fn group_name(group: &EditorGroup, index: usize) -> String {
    if group.name.is_empty() {
        format!("Group #{}", index)
    } else {
        format!("Group \"{}\"", group.name)
    }
}

pub fn layer_name(resources: &EditorResources, layer: &EditorLayer, index: usize) -> String {
    let icon = match layer {
        MapLayerSkeleton::Abritrary(_) => "\u{f057}",
        MapLayerSkeleton::Tile(_) => "\u{f00a}",
        MapLayerSkeleton::Quad(_) => "\u{f61f}",
        MapLayerSkeleton::Sound(_) => "\u{1f3b5}",
    };
    if !layer.name().is_empty() {
        format!("{icon} Layer \"{}\"", layer.name())
    } else if let Some(text) = match layer {
        MapLayerSkeleton::Abritrary(_) => Some("\u{f057} unsupported".to_string()),
        MapLayerSkeleton::Tile(layer) => layer.layer.attr.image_array.map(|image| {
            format!(
                "\u{f302} {}",
                resources.image_arrays[image].def.name.as_str()
            )
        }),
        MapLayerSkeleton::Quad(layer) => layer
            .layer
            .attr
            .image
            .map(|image| format!("\u{f03e} {}", resources.images[image].def.name.as_str())),
        MapLayerSkeleton::Sound(layer) => layer
            .layer
            .attr
            .sound
            .map(|sound| format!("\u{1f3b5} {}", resources.sounds[sound].def.name.as_str())),
    } {
        format!(" Layer \"{}\"", text)
    } else {
        format!(" Layer #{}", index)
    }
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
