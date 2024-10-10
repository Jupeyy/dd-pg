use std::path::PathBuf;

use base::hash::fmt_hash;
use egui::{Button, DragValue};
use egui_file_dialog::{DialogMode, DialogState};
use network::network::utils::create_certifified_keys;
use ui_base::types::UiRenderPipe;

use crate::{
    explain::TEXT_ANIM_PANEL_AND_PROPS,
    ui::{
        user_data::{
            EditorMenuDialogMode, EditorMenuHostDialogMode, EditorMenuHostNetworkOptions,
            EditorUiEvent, EditorUiEventHostMap, UserData,
        },
        utils::icon_font_text,
    },
};

pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>, main_frame_only: bool) {
    let style = ui.style();
    let height = style.spacing.interact_size.y + style.spacing.item_spacing.y;
    egui::TopBottomPanel::top("top_menu")
        .resizable(false)
        .default_height(height)
        .height_range(height..=height)
        .show_inside(ui, |ui| {
            egui::ScrollArea::horizontal().show(ui, |ui| {
                let menu_dialog_mode = &mut *pipe.user_data.menu_dialog_mode;

                ui.horizontal(|ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("Open map").clicked() {
                            *menu_dialog_mode = EditorMenuDialogMode::open(pipe.user_data.io);
                        }
                        if ui.button("Save map").clicked() {
                            *menu_dialog_mode = EditorMenuDialogMode::save(pipe.user_data.io);
                        }
                        if ui.button("Host map").clicked() {
                            *menu_dialog_mode = EditorMenuDialogMode::host(pipe.user_data.io);
                        }
                        if ui.button("Join map").clicked() {
                            *menu_dialog_mode = EditorMenuDialogMode::join();
                        }
                        if ui.button("Close").clicked() {
                            pipe.user_data.ui_events.push(EditorUiEvent::Close);
                        }
                    });

                    ui.menu_button("Tools", |ui| {
                        if ui.button("Automapper-Creator").clicked() {
                            pipe.user_data.auto_mapper.active = true;
                        }
                    });

                    if let Some(tab) = &mut pipe.user_data.editor_tab {
                        ui.menu_button(icon_font_text(ui, "\u{f013}"), |ui| {
                            let btn = Button::new("Disable animations panel + properties")
                                .selected(tab.map.user.options.no_animations_with_properties);
                            if ui
                                .add(btn)
                                .on_hover_ui(|ui| {
                                    let mut cache = egui_commonmark::CommonMarkCache::default();
                                    egui_commonmark::CommonMarkViewer::new().show(
                                        ui,
                                        &mut cache,
                                        TEXT_ANIM_PANEL_AND_PROPS,
                                    );
                                })
                                .clicked()
                            {
                                tab.map.user.options.no_animations_with_properties =
                                    !tab.map.user.options.no_animations_with_properties;
                            }
                            let btn = Button::new("Show tile layer indices")
                                .selected(tab.map.user.options.show_tile_numbers);
                            if ui.add(btn).clicked() {
                                tab.map.user.options.show_tile_numbers =
                                    !tab.map.user.options.show_tile_numbers;
                            }
                        });
                    }
                });

                if let EditorMenuDialogMode::Open { file_dialog }
                | EditorMenuDialogMode::Save { file_dialog }
                | EditorMenuDialogMode::Host {
                    mode: EditorMenuHostDialogMode::SelectMap { file_dialog },
                } = menu_dialog_mode
                {
                    if !main_frame_only && file_dialog.state() == DialogState::Open {
                        let mode = file_dialog.mode();
                        if let Some(selected) = file_dialog.update(ui.ctx()).selected() {
                            let selected: PathBuf = selected.into();
                            if let EditorMenuDialogMode::Open { .. }
                            | EditorMenuDialogMode::Save { .. } = menu_dialog_mode
                            {
                                match mode {
                                    DialogMode::SelectFile => {
                                        pipe.user_data
                                            .ui_events
                                            .push(EditorUiEvent::OpenFile { name: selected });
                                    }
                                    DialogMode::SelectDirectory | DialogMode::SelectMultiple => {
                                        todo!()
                                    }
                                    DialogMode::SaveFile => {
                                        pipe.user_data
                                            .ui_events
                                            .push(EditorUiEvent::SaveFile { name: selected });
                                    }
                                }
                            } else if let EditorMenuDialogMode::Host { mode } = menu_dialog_mode {
                                let (cert, private_key) = create_certifified_keys();

                                *mode = EditorMenuHostDialogMode::HostNetworkOptions(Box::new(
                                    EditorMenuHostNetworkOptions {
                                        map_path: selected,
                                        port: 0,
                                        password: Default::default(),
                                        cert,
                                        private_key,
                                    },
                                ));
                            }
                        }
                    }
                }

                if let EditorMenuDialogMode::Host {
                    mode: EditorMenuHostDialogMode::HostNetworkOptions(mode),
                } = menu_dialog_mode
                {
                    let EditorMenuHostNetworkOptions {
                        port,
                        password,
                        cert,
                        ..
                    } = mode.as_mut();
                    if !main_frame_only {
                        let window = egui::Window::new("Host map network options")
                            .resizable(false)
                            .collapsible(false);

                        let mut host = false;
                        let mut cancel = false;
                        let window_res = window.show(ui.ctx(), |ui| {
                            ui.label("Port: (0 = random port)");
                            ui.add(DragValue::new(port));

                            ui.label("Certificate hash:");
                            // TODO: cache this
                            let hash = cert
                                .tbs_certificate
                                .subject_public_key_info
                                .fingerprint_bytes()
                                .unwrap();
                            ui.label(fmt_hash(&hash));

                            ui.label("Password:");
                            ui.text_edit_singleline(password);
                            if ui.button("Host").clicked() {
                                host = true;
                            }
                            if ui.button("Cancel").clicked() {
                                cancel = true;
                            }
                        });

                        if host {
                            let EditorMenuDialogMode::Host {
                                mode: EditorMenuHostDialogMode::HostNetworkOptions(mode),
                            } = std::mem::replace(menu_dialog_mode, EditorMenuDialogMode::None)
                            else {
                                return;
                            };
                            let EditorMenuHostNetworkOptions {
                                port,
                                password,
                                map_path,
                                cert,
                                private_key,
                            } = *mode;
                            pipe.user_data
                                .ui_events
                                .push(EditorUiEvent::HostMap(Box::new(EditorUiEventHostMap {
                                    map_path,
                                    port,
                                    password,
                                    cert,
                                    private_key,
                                })));
                        } else if cancel {
                            *menu_dialog_mode = EditorMenuDialogMode::None;
                        }

                        *pipe.user_data.pointer_is_used |= if let Some(window_res) = window_res {
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
                            if intersected.is_some_and(|(outside, clicked)| outside && clicked) {
                                *menu_dialog_mode = EditorMenuDialogMode::None;
                            }
                            intersected.is_some_and(|(outside, _)| !outside)
                        } else {
                            false
                        };
                    }
                } else if let EditorMenuDialogMode::Join {
                    ip_port,
                    cert_hash,
                    password,
                } = menu_dialog_mode
                {
                    if !main_frame_only {
                        let window = egui::Window::new("Join map network options")
                            .resizable(false)
                            .collapsible(false);

                        let mut join = false;
                        let mut cancel = false;
                        let window_res = window.show(ui.ctx(), |ui| {
                            ui.label("Address (IP:PORT)");
                            ui.text_edit_singleline(ip_port);
                            ui.label("Certificate hash:");
                            ui.text_edit_singleline(cert_hash);
                            ui.label("Password:");
                            ui.text_edit_singleline(password);
                            if ui.button("Join").clicked() {
                                join = true;
                            }
                            if ui.button("Cancel").clicked() {
                                cancel = true;
                            }
                        });

                        if join {
                            let EditorMenuDialogMode::Join {
                                ip_port,
                                cert_hash,
                                password,
                            } = std::mem::replace(menu_dialog_mode, EditorMenuDialogMode::None)
                            else {
                                return;
                            };
                            pipe.user_data.ui_events.push(EditorUiEvent::Join {
                                ip_port,
                                cert_hash,
                                password,
                            });
                        } else if cancel {
                            *menu_dialog_mode = EditorMenuDialogMode::None;
                        }

                        *pipe.user_data.pointer_is_used |= if let Some(window_res) = window_res {
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
                            if intersected.is_some_and(|(outside, clicked)| outside && clicked) {
                                *menu_dialog_mode = EditorMenuDialogMode::None;
                            }
                            intersected.is_some_and(|(outside, _)| !outside)
                        } else {
                            false
                        };
                    }
                }

                if !main_frame_only && pipe.user_data.auto_mapper.active {
                    crate::ui::auto_mapper::auto_mapper::render(main_frame_only, pipe, ui);
                }
            });
        });
}
