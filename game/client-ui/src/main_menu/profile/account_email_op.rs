use std::{str::FromStr, sync::Arc};

use base_io::io::Io;
use config::config::ConfigPath;
use egui::Label;
use ui_base::utils::icon_font_plus_text;

use crate::main_menu::{
    profiles_interface::ProfilesInterface,
    user_data::{AccountOperation, ProfileState, ProfileTasks},
};

use super::back_bar::back_bar;

/// overview
pub fn render(
    ui: &mut egui::Ui,
    accounts: &Arc<dyn ProfilesInterface>,
    tasks: &mut ProfileTasks,
    io: &Io,
    path: &mut ConfigPath,
) {
    if let ProfileState::EmailAccountTokenObtained(op) = &tasks.state {
        let op = op.clone();
        back_bar(
            ui,
            match &op {
                AccountOperation::LogoutAll { .. } => "Logout all by email",
                AccountOperation::LinkCredential { .. } => "Verify new link by email",
                AccountOperation::Delete { .. } => "Delete account by email",
            },
            tasks,
        );

        ui.vertical_centered(|ui| {
            if path
                .query
                .get("email")
                .and_then(|email| email_address::EmailAddress::from_str(email).ok())
                .is_some()
            {
                egui::Grid::new("by-email")
                    .spacing([2.0, 4.0])
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Account's Email:");

                        let email = path.query.entry("email".into()).or_default();
                        ui.add_enabled(false, Label::new(email.as_str()));
                        ui.end_row();

                        ui.label("Code:");

                        let token = path.query.entry("token".into()).or_default();
                        ui.text_edit_singleline(token);
                        ui.end_row();
                    });

                if ui
                    .button(icon_font_plus_text(
                        ui,
                        "\u{f2f6}",
                        match &op {
                            AccountOperation::LogoutAll { .. } => "Logout all",
                            AccountOperation::LinkCredential { .. } => "Verify",
                            AccountOperation::Delete { .. } => "Delete account",
                        },
                    ))
                    .clicked()
                {
                    if let Some((_, token)) = path
                        .query
                        .get("email")
                        .and_then(|email| email_address::EmailAddress::from_str(email).ok())
                        .zip(path.query.get("token").cloned())
                    {
                        path.query.remove("token");
                        path.query.remove("email");
                        path.query.remove("veri-token");
                        let accounts = accounts.clone();
                        match op {
                            AccountOperation::LogoutAll { profile_name } => {
                                tasks.state = ProfileState::EmailLogoutAll(
                                    io.io_batcher
                                        .spawn(async move {
                                            accounts.logout_all(token, &profile_name).await
                                        })
                                        .abortable(),
                                );
                            }
                            AccountOperation::LinkCredential {
                                credential_auth_token,
                                profile_name,
                            } => {
                                tasks.state = ProfileState::EmailLinkCredential(
                                    io.io_batcher
                                        .spawn(async move {
                                            accounts
                                                .link_credential(
                                                    token,
                                                    credential_auth_token,
                                                    &profile_name,
                                                )
                                                .await
                                        })
                                        .abortable(),
                                );
                            }
                            AccountOperation::Delete { profile_name } => {
                                tasks.state = ProfileState::EmailDelete(
                                    io.io_batcher
                                        .spawn(async move {
                                            accounts.delete(token, &profile_name).await
                                        })
                                        .abortable(),
                                );
                            }
                        }
                    }
                }
            }
        });
    }
}
