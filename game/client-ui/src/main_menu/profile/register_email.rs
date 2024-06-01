use std::{str::FromStr, sync::Arc};

use base_io::io::Io;
use config::config::ConfigPath;
use egui::Color32;
use ui_base::utils::{self, icon_font_plus_text};

use crate::main_menu::{profiles_interface::ProfilesInterface, user_data::ProfileTasks};

use super::{
    back_bar::back_bar,
    password_strength::{password_strength, PasswordStrengthScore},
};

/// overview
pub fn render(
    ui: &mut egui::Ui,
    accounts: &Arc<dyn ProfilesInterface>,
    tasks: &mut ProfileTasks,
    io: &Io,
    path: &mut ConfigPath,
) {
    back_bar(ui, "Register with email", path);
    ui.vertical_centered(|ui| {
        egui::Grid::new("register-by-email")
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

                if let Err(err) = email_address::EmailAddress::from_str(name) {
                    ui.label("error:");
                    ui.label(err.to_string());
                }
                ui.end_row();

                ui.label("Password:");

                let password = path
                    .query
                    .entry("password".into())
                    .or_insert_with(|| Default::default());
                ui.add(utils::password(password));
                ui.end_row();

                let score = password_strength(password);
                let color = match score {
                    PasswordStrengthScore::VeryWeak => Color32::RED,
                    PasswordStrengthScore::Weak => Color32::LIGHT_RED,
                    PasswordStrengthScore::StillWeak => Color32::YELLOW,
                    PasswordStrengthScore::Ok => Color32::LIGHT_GREEN,
                    PasswordStrengthScore::Strong => Color32::GREEN,
                };
                ui.painter()
                    .rect_filled(ui.available_rect_before_wrap(), 5.0, color);
                ui.label("");
                ui.label(format!(
                    "Password is {}",
                    match score {
                        PasswordStrengthScore::VeryWeak => "very weak",
                        PasswordStrengthScore::Weak => "weak",
                        PasswordStrengthScore::StillWeak => "still too weak",
                        PasswordStrengthScore::Ok => "ok",
                        PasswordStrengthScore::Strong => "strong",
                    }
                ));
                ui.end_row();
            });

        if ui
            .button(icon_font_plus_text(ui, "\u{f2c2}", "Register"))
            .clicked()
        {
            if let Some((email, password)) = path
                .query
                .get("name")
                .map(|email| email_address::EmailAddress::from_str(email).ok())
                .flatten()
                .zip(path.query.get("password"))
            {
                let score = password_strength(password);
                match score {
                    PasswordStrengthScore::Ok | PasswordStrengthScore::Strong => {
                        let password = password.clone();
                        let accounts = accounts.clone();
                        tasks.register.push(
                            io.io_batcher
                                .spawn(async move { accounts.register(email, &password).await })
                                .abortable(),
                        );
                    }
                    _ => {
                        // ignore
                    }
                }
            }
        }
    });
}
