use std::sync::Arc;

use base_io::io::Io;

use crate::main_menu::{
    profiles_interface::{CredentialAuthTokenOperation, ProfilesInterface},
    user_data::{CredentialAuthOperation, ProfileState, ProfileTasks},
};

use super::back_bar::back_bar;

/// overview
pub fn render(
    ui: &mut egui::Ui,
    accounts: &Arc<dyn ProfilesInterface>,
    tasks: &mut ProfileTasks,
    io: &Io,
) {
    if let ProfileState::SteamCredentialAuthTokenPrepare(op) = &tasks.state {
        let op = op.clone();
        back_bar(
            ui,
            match &op {
                CredentialAuthOperation::Login => "Login by steam",
                CredentialAuthOperation::LinkCredential { .. } => "Link new steam",
                CredentialAuthOperation::UnlinkCredential { .. } => "Unlink steam",
            },
            tasks,
        );
        let accounts = accounts.clone();
        tasks.state = ProfileState::SteamCredentialAuthToken {
            op: op.clone(),
            task: io
                .io_batcher
                .spawn(async move {
                    Ok(accounts
                        .credential_auth_steam_token(
                            match op {
                                CredentialAuthOperation::Login => {
                                    CredentialAuthTokenOperation::Login
                                }
                                CredentialAuthOperation::LinkCredential { .. } => {
                                    CredentialAuthTokenOperation::LinkCredential
                                }
                                CredentialAuthOperation::UnlinkCredential { .. } => {
                                    CredentialAuthTokenOperation::UnlinkCredential
                                }
                            },
                            None,
                        )
                        .await)
                })
                .abortable(),
        };
    }
}
