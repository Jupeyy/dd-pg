use std::sync::Arc;

use base_io::io::Io;
use egui::{Layout, ScrollArea};
use egui_extras::{Size, StripBuilder};
use ui_base::utils::{icon_font_plus_text, icon_font_text_for_btn};

use crate::main_menu::{
    profiles_interface::ProfilesInterface,
    user_data::{CredentialAuthOperation, ProfileState, ProfileTasks},
};

/// overview
pub fn render(
    ui: &mut egui::Ui,
    accounts: &Arc<dyn ProfilesInterface>,
    tasks: &mut ProfileTasks,
    io: &Io,
) {
    ui.vertical_centered(|ui| {
        ui.label("Profiles");

        StripBuilder::new(ui)
            .size(Size::remainder())
            .size(Size::exact(200.0))
            .horizontal(|mut strip| {
                strip.cell(|ui| {
                    ui.set_width(ui.available_width());
                    ui.set_height(ui.available_height());
                    ui.label("Active accounts:");
                    ScrollArea::vertical().show(ui, |ui| {
                        ui.with_layout(
                            Layout::top_down(egui::Align::Min).with_cross_justify(true),
                            |ui| {
                                let (profiles, cur_profile) = accounts.profiles();
                                let mut profiles: Vec<_> = profiles.into_iter().collect();
                                profiles.sort_by_key(|(key, _)| key.clone());
                                for (key, account) in profiles {
                                    ui.with_layout(Layout::right_to_left(egui::Align::Min), |ui| {
                                        if ui
                                            .button(icon_font_text_for_btn(ui, "\u{f129}"))
                                            .clicked()
                                        {
                                            let profile = key.to_string();
                                            let profile_data = account.clone();
                                            let accounts = accounts.clone();
                                            tasks.state = ProfileState::AccountInfoFetch {
                                                task: io
                                                    .io_batcher
                                                    .spawn(async move {
                                                        accounts.account_info(&profile).await
                                                    })
                                                    .abortable(),
                                                profile_name: key.to_string(),
                                                profile_data: profile_data.clone(),
                                            };
                                        }
                                        ui.with_layout(
                                            Layout::left_to_right(egui::Align::Min)
                                                .with_main_justify(true),
                                            |ui| {
                                                if ui
                                                    .selectable_label(
                                                        key.as_str() == cur_profile.as_str(),
                                                        &account.name,
                                                    )
                                                    .clicked()
                                                {
                                                    let profile = key.to_string();
                                                    let accounts = accounts.clone();
                                                    tasks.user_interactions.push(
                                                        io.io_batcher
                                                            .spawn(async move {
                                                                accounts
                                                                    .set_profile(&profile)
                                                                    .await;
                                                                Ok(())
                                                            })
                                                            .abortable(),
                                                    );
                                                }
                                            },
                                        );
                                    });
                                }
                            },
                        );
                    });
                });
                strip.cell(|ui| {
                    ui.with_layout(Layout::top_down_justified(egui::Align::Center), |ui| {
                        if ui
                            .button(icon_font_plus_text(ui, "\u{f0e0}", "Login with email"))
                            .clicked()
                        {
                            tasks.state = ProfileState::EmailCredentialAuthTokenPrepare(
                                CredentialAuthOperation::Login,
                            );
                        }
                        if accounts.supports_steam()
                            && ui
                                .button(icon_font_plus_text(ui, "\u{2b}", "Login with Steam"))
                                .clicked()
                        {
                            tasks.state = ProfileState::SteamCredentialAuthTokenPrepare(
                                CredentialAuthOperation::Login,
                            );
                        }
                    });
                });
            });
    });
}
