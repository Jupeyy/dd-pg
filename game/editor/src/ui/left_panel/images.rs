use base_io::io::IO;
use map::map::{resources::MapResourceRef, Map};
use ui_base::types::UIState;

use crate::{
    actions::actions::{
        ActAddImage, ActAddRemImage, ActChangeQuadLayerAttr, ActRemImage, EditorAction,
        EditorActionGroup,
    },
    client::EditorClient,
    map::{EditorGroup, EditorGroupPanelResources, EditorGroups, EditorLayer, EditorResources},
};

pub fn render(
    ui: &mut egui::Ui,
    ui_state: &mut UIState,
    main_frame_only: bool,
    client: &mut EditorClient,
    groups: &EditorGroups,
    resources: &mut EditorResources,
    panel_data: &mut EditorGroupPanelResources,
    io: &IO,
) {
    super::resource_panel::render(
        ui,
        ui_state,
        main_frame_only,
        client,
        &mut resources.images,
        panel_data,
        io,
        |client, images, name, file| {
            let ty = name.extension().unwrap().to_string_lossy().to_string();
            let (name, hash) =
                Map::name_and_hash(&name.file_stem().unwrap().to_string_lossy(), &file);

            client.execute(
                EditorAction::AddImage(ActAddImage {
                    base: ActAddRemImage {
                        res: MapResourceRef {
                            name,
                            blake3_hash: hash,
                            ty,
                        },
                        file,
                        index: images.len(),
                    },
                }),
                None,
            );
        },
        |client, images, index| {
            let mut actions = Vec::new();
            let mut change_layers = |groups: &Vec<EditorGroup>, is_background: bool| {
                for (g, group) in groups.iter().enumerate() {
                    for (l, layer) in group.layers.iter().enumerate() {
                        if let EditorLayer::Quad(layer) = layer {
                            if layer.layer.attr.image >= Some(index) {
                                let mut attr = layer.layer.attr.clone();
                                attr.image = if layer.layer.attr.image == Some(index) {
                                    None
                                } else {
                                    layer.layer.attr.image.map(|index| index - 1)
                                };
                                actions.push(EditorAction::ChangeQuadLayerAttr(
                                    ActChangeQuadLayerAttr {
                                        is_background,
                                        group_index: g,
                                        layer_index: l,
                                        old_attr: layer.layer.attr.clone(),
                                        new_attr: attr,
                                    },
                                ));
                            }
                        }
                    }
                }
            };

            change_layers(&groups.background, true);
            change_layers(&groups.foreground, false);

            actions.push(EditorAction::RemImage(ActRemImage {
                base: ActAddRemImage {
                    res: images[index].def.clone(),
                    file: images[index].user.file.as_ref().clone(),
                    index,
                },
            }));
            client.execute_group(EditorActionGroup {
                actions,
                identifier: None,
            })
        },
    );
}
