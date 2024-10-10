use std::sync::Arc;

use base_io::io::Io;

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
) {
    if let ProfileState::SteamAccountTokenPrepare(op) = &tasks.state {
        let op = op.clone();
        back_bar(
            ui,
            match &op {
                AccountOperation::LogoutAll { .. } => "Logout all by steam",
                AccountOperation::LinkCredential { .. } => "Verify new link by steam",
                AccountOperation::Delete { .. } => "Delete account by steam",
            },
            tasks,
        );
        let accounts = accounts.clone();
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
                                AccountOperation::Delete { .. } => AccountTokenOperation::Delete,
                            },
                            None,
                        )
                        .await)
                })
                .abortable(),
        };
    }
}
