use std::{str::FromStr, sync::Arc};

use base_io::io::Io;
use config::config::ConfigPath;
use ui_base::utils::{self, icon_font_plus_text};

use crate::main_menu::{profiles_interface::ProfilesInterface, user_data::ProfileTasks};

use super::back_bar::back_bar;

/// overview
pub fn render(
    ui: &mut egui::Ui,
    accounts: &Arc<dyn ProfilesInterface>,
    tasks: &mut ProfileTasks,
    io: &Io,
    path: &mut ConfigPath,
) {
    back_bar(ui, "Login by email", path);
    ui.vertical_centered(|ui| {
        egui::Grid::new("login-by-email")
            .spacing([2.0, 4.0])
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("Email:");

                let name = path
                    .query
                    .entry("name".into())
                    .or_insert_with(|| Default::default());
                ui.text_edit_singleline(name);
                ui.end_row();

                ui.label("Password:");

                let password = path
                    .query
                    .entry("password".into())
                    .or_insert_with(|| Default::default());
                ui.add(utils::password(password));
                ui.end_row();
            });

        if ui
            .button(icon_font_plus_text(ui, "\u{f2f6}", "Login"))
            .clicked()
        {
            if let Some((email, password)) = path
                .query
                .get("name")
                .map(|email| email_address::EmailAddress::from_str(email).ok())
                .flatten()
                .zip(path.query.get("password"))
            {
                let password = password.clone();
                let accounts = accounts.clone();
                tasks.login.push(
                    io.io_batcher
                        .spawn(async move { accounts.login(email, &password).await })
                        .abortable(),
                );
            }
        }
    });
}
