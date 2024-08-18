use std::{str::FromStr, sync::Arc};

use base_io::io::Io;
use config::config::ConfigPath;
use egui::{Label, ScrollArea, Spinner};
use ui_base::utils::icon_font_plus_text;

use crate::main_menu::{profiles_interface::ProfilesInterface, user_data::ProfileTasks};

use super::{back_bar::back_bar, constants::PROFILE_PAGE_QUERY};

/// overview
pub fn render(
    ui: &mut egui::Ui,
    accounts: &Arc<dyn ProfilesInterface>,
    tasks: &mut ProfileTasks,
    io: &Io,
    path: &mut ConfigPath,
) {
    back_bar(ui, "Login by email", path);

    let loading = tasks.login_tokens.iter().any(|task| !task.is_finished());
    if loading {
        ui.add(Spinner::new());
    } else {
        for task in tasks.login_tokens.drain(..) {
            if let Err(err) = task.get_storage() {
                tasks.errors.push(err.to_string());
            }
        }

        if !tasks.errors.is_empty() {
            ui.label("the following errors occurred:");
            ScrollArea::vertical().show(ui, |ui| {
                for err in &tasks.errors {
                    ui.label(err);
                }
            });
            if ui.button("try again").clicked() {
                tasks.errors.clear();
                path.route_query_only_single((PROFILE_PAGE_QUERY.into(), "".to_string()));
            }
        } else {
            ui.vertical_centered(|ui| {
                if path
                    .query
                    .get("email")
                    .and_then(|email| email_address::EmailAddress::from_str(email).ok())
                    .is_some()
                {
                    egui::Grid::new("login-by-email")
                        .spacing([2.0, 4.0])
                        .num_columns(2)
                        .show(ui, |ui| {
                            ui.label("Email:");

                            let email = path.query.entry("email".into()).or_default();
                            ui.add_enabled(false, Label::new(email.as_str()));
                            ui.end_row();

                            ui.label("Token:");

                            let token = path.query.entry("token".into()).or_default();
                            ui.text_edit_singleline(token);
                            ui.end_row();
                        });

                    if ui
                        .button(icon_font_plus_text(ui, "\u{f2f6}", "Login"))
                        .clicked()
                    {
                        if let Some((email, token)) = path
                            .query
                            .get("email")
                            .and_then(|email| email_address::EmailAddress::from_str(email).ok())
                            .zip(path.query.get("token").cloned())
                        {
                            let accounts = accounts.clone();
                            tasks.logins.push(
                                io.io_batcher
                                    .spawn(async move { accounts.login_email(email, token).await })
                                    .abortable(),
                            );
                            path.route_query_only_single((
                                PROFILE_PAGE_QUERY.into(),
                                "finish-login".to_string(),
                            ));
                        }
                    }
                }
            });
        }
    }
}
