use std::sync::Arc;

use base_io::io::Io;
use config::config::ConfigPath;
use ui_base::utils::icon_font_plus_text;

use crate::main_menu::{
    profiles_interface::{AccountTokenOperation, ProfilesInterface},
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
    if let ProfileState::SteamAccountTokenWebValidation { op, url } = &tasks.state {
        let op = op.clone();
        let url = url.clone();
        back_bar(
            ui,
            match &op {
                AccountOperation::LogoutAll { .. } => "Logout all by steam",
                AccountOperation::LinkCredential { .. } => "Verify new link by steam",
                AccountOperation::Delete { .. } => "Delete account by steam",
            },
            tasks,
        );
        ui.vertical_centered(|ui| {
            ui.label("A verification on this web page is needed:");
            ui.hyperlink(url);
            ui.label("Afterwards add the code from\nthe web page to this field:");
            egui::Grid::new("login-steam-token-secret-key")
                .spacing([2.0, 4.0])
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("Token:");
                    let veri_token = path.query.entry("veri-token".into()).or_default();
                    ui.text_edit_singleline(veri_token);
                    ui.end_row();
                });

            if ui
                .button(icon_font_plus_text(ui, "\u{f2f6}", "Request code by steam"))
                .clicked()
            {
                let veri_token = path.query.get("veri-token");

                let accounts = accounts.clone();
                let veri_token = veri_token.cloned();
                tasks.state = ProfileState::SteamAccountToken {
                    op: op.clone(),
                    task: io
                        .io_batcher
                        .spawn(async move {
                            Ok(accounts
                                .account_steam_token(
                                    match op {
                                        AccountOperation::LogoutAll { .. } => {
                                            AccountTokenOperation::LogoutAll
                                        }
                                        AccountOperation::LinkCredential { .. } => {
                                            AccountTokenOperation::LinkCredential
                                        }
                                        AccountOperation::Delete { .. } => {
                                            AccountTokenOperation::Delete
                                        }
                                    },
                                    veri_token,
                                )
                                .await)
                        })
                        .abortable(),
                };
            }
        });
    }
}
