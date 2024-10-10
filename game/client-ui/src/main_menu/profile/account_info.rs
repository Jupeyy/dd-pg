use std::sync::Arc;

use base_io::io::Io;
use egui::{Grid, Layout};
use egui_extras::{Size, StripBuilder};
use ui_base::utils::icon_font_text_for_btn;

use crate::main_menu::{
    profiles_interface::{LinkedCredential, ProfilesInterface},
    user_data::{ProfileState, ProfileTasks},
};

use super::back_bar::back_bar;

/// overview
pub fn render(
    ui: &mut egui::Ui,
    accounts: &Arc<dyn ProfilesInterface>,
    tasks: &mut ProfileTasks,
    io: &Io,
) {
    back_bar(ui, "Account overview", tasks);

    if let ProfileState::AccountInfo {
        info,
        profile_name,
        profile_data,
    } = &mut tasks.state
    {
        let mut next_state = None;
        Grid::new("account_info").num_columns(2).show(ui, |ui| {
            ui.label("Profile name:");
            StripBuilder::new(ui)
                .size(Size::remainder())
                .size(Size::exact(30.0))
                .horizontal(|mut strip| {
                    strip.cell(|ui| {
                        ui.text_edit_singleline(&mut profile_data.name);
                    });
                    strip.cell(|ui| {
                        if ui.button(icon_font_text_for_btn(ui, "\u{f00c}")).clicked() {
                            let accounts = accounts.clone();
                            let display_name = profile_data.name.clone();
                            let profile_name = profile_name.clone();
                            tasks.user_interactions.push(
                                io.io_batcher
                                    .spawn(async move {
                                        accounts
                                            .set_profile_display_name(&profile_name, display_name)
                                            .await;
                                        Ok(())
                                    })
                                    .abortable(),
                            );
                        }
                    });
                });
            ui.end_row();
            ui.label("Account id:");
            ui.label(info.account_id.to_string());
            ui.end_row();
            ui.label("Creation date:");
            ui.label(&info.creation_date);
            ui.end_row();
            let can_unlink = info.credentials.len() >= 2;
            for credential in info.credentials.iter() {
                match credential {
                    LinkedCredential::Email(mail) => {
                        ui.label("Linked email:");
                        StripBuilder::new(ui)
                            .size(Size::remainder())
                            .size(Size::exact(30.0))
                            .horizontal(|mut strip| {
                                strip.cell(|ui| {
                                    ui.label(mail);
                                });
                                strip.cell(|ui| {
                                    if can_unlink
                                        && ui
                                            .button(icon_font_text_for_btn(ui, "\u{f1f8}"))
                                            .clicked()
                                    {
                                        let profile_name = profile_name.clone();
                                        next_state = Some(ProfileState::UnlinkEmailPrepare {
                                            profile_name,
                                            info: info.clone(),
                                        });
                                    }
                                });
                            });
                    }
                    LinkedCredential::Steam(id) => {
                        ui.label("Linked steam id:");
                        StripBuilder::new(ui)
                            .size(Size::remainder())
                            .size(Size::exact(30.0))
                            .horizontal(|mut strip| {
                                strip.cell(|ui| {
                                    ui.label(id.to_string());
                                });
                                strip.cell(|ui| {
                                    if can_unlink
                                        && accounts.steam_id64() == *id
                                        && ui
                                            .button(icon_font_text_for_btn(ui, "\u{f1f8}"))
                                            .clicked()
                                    {
                                        let profile_name = profile_name.clone();
                                        next_state = Some(ProfileState::UnlinkSteamPrepare {
                                            profile_name,
                                            info: info.clone(),
                                        });
                                    }
                                });
                            });
                    }
                }
                ui.end_row();
            }
        });

        ui.with_layout(
            Layout::left_to_right(egui::Align::Min).with_main_wrap(true),
            |ui| {
                if ui.button("Logout").clicked() {
                    let profile_name = profile_name.clone();
                    let accounts = accounts.clone();
                    next_state = Some(ProfileState::Logout(
                        io.io_batcher
                            .spawn(async move { accounts.logout(&profile_name).await })
                            .abortable(),
                    ));
                }
                if ui.button("Logout other sessions").clicked() {
                    let profile_name = profile_name.clone();
                    next_state = Some(ProfileState::LogoutAllPrepare {
                        profile_name,
                        info: info.clone(),
                    });
                }
                if ui.button("Delete account").clicked() {
                    let profile_name = profile_name.clone();
                    next_state = Some(ProfileState::DeleteConfirm {
                        profile_name,
                        info: info.clone(),
                    });
                }
                if !info
                    .credentials
                    .iter()
                    .any(|c| matches!(c, LinkedCredential::Email(_)))
                    && ui.button("Link email").clicked()
                {
                    let profile_name = profile_name.clone();
                    next_state = Some(ProfileState::LinkEmailPrepare {
                        profile_name,
                        info: info.clone(),
                    });
                }
                if accounts.supports_steam()
                    && !info
                        .credentials
                        .iter()
                        .any(|c| matches!(c, LinkedCredential::Steam(_)))
                    && ui.button("Link steam").clicked()
                {
                    let profile_name = profile_name.clone();
                    next_state = Some(ProfileState::LinkSteamPrepare {
                        profile_name,
                        info: info.clone(),
                    });
                }
            },
        );
        if let Some(next_state) = next_state {
            tasks.state = next_state;
        }
    }
}
