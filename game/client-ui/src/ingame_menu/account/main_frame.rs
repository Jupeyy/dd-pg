use egui::{epaint::RectShape, Color32, Frame, Shape, TextEdit};

use game_interface::types::network_string::NetworkReducedAsciiString;
use ui_base::{types::UiRenderPipe, utils::add_horizontal_margins};

use crate::{
    events::{UiEvent, UiEvents},
    ingame_menu::{account_info::AccountInfo, user_data::UserData},
};

fn render_name_change(ui: &mut egui::Ui, events: &UiEvents, account_info: &AccountInfo) {
    let mut account_name = account_info.edit_data();
    ui.add(TextEdit::singleline(&mut account_name).char_limit(24));
    let account_name_res = NetworkReducedAsciiString::new(account_name.as_str());
    account_info.fill_edit_data(account_name);
    match account_name_res {
        Ok(new_account_name) if new_account_name.len() >= 3 => {
            if ui.button("Change name").clicked() {
                events.push(UiEvent::ChangeAccountName {
                    name: new_account_name,
                });
            }
        }
        Ok(_) => {
            ui.colored_label(
                Color32::RED,
                "Name must be at least \
                3 characters long.",
            );
        }
        Err(err) => {
            ui.colored_label(Color32::RED, err.to_string());
        }
    }

    let action_response = account_info.last_action_response();
    if let Some(action_response) = action_response {
        match action_response {
            Some(err) => {
                ui.colored_label(Color32::RED, err);
            }
            None => {
                ui.colored_label(Color32::GREEN, "Your request was successful.");
            }
        }
    }

    ui.add_space(10.0);

    let account_info = account_info.account_info();
    ui.label("Account information on the server");

    match account_info {
        Some((account_info, creation_date)) => {
            ui.label(format!("Name: {}", account_info.name.as_str()));
            ui.label(format!("Creation date: {creation_date}"));
        }
        None => {
            ui.label("Loading...");
            events.push(UiEvent::RequestAccountInfo);
        }
    }
}

/// main frame. full width
pub fn render(ui: &mut egui::Ui, pipe: &mut UiRenderPipe<UserData>, main_frame_only: bool) {
    if main_frame_only {
        ui.painter().add(Shape::Rect(RectShape::filled(
            ui.available_rect_before_wrap(),
            0.0,
            Color32::from_rgba_unmultiplied(0, 0, 0, 255),
        )));
    } else {
        Frame::default()
            .fill(Color32::from_rgba_unmultiplied(0, 0, 0, 100))
            .show(ui, |ui| {
                add_horizontal_margins(ui, |ui| {
                    ui.label(
                        "Here you can modify your account name \
                        for all game servers this server belongs to.",
                    );

                    render_name_change(
                        ui,
                        pipe.user_data.browser_menu.events,
                        pipe.user_data.account_info,
                    );
                });
            });
    }
}
