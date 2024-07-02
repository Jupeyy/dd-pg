use std::sync::Arc;

use config::config::ConfigPath;
use egui::{Color32, ScrollArea};
use egui_extras::{Size, StripBuilder};
use ui_base::utils::icon_font_plus_text;

use crate::main_menu::{profiles_interface::ProfilesInterface, user_data::ProfileTasks};

use super::constants::PROFILE_PAGE_QUERY;

/// overview
pub fn render(
    ui: &mut egui::Ui,
    accounts: &Arc<dyn ProfilesInterface>,
    tasks: &ProfileTasks,
    path: &mut ConfigPath,
) {
    ui.vertical_centered(|ui| {
        ui.label("Profiles");

        if !tasks.errors.is_empty() {
            ScrollArea::vertical().show(ui, |ui| {
                ui.vertical(|ui| {
                    for err in tasks.errors.iter() {
                        ui.label(err);
                    }
                })
            });
        }

        StripBuilder::new(ui)
            .size(Size::remainder())
            .size(Size::remainder())
            .horizontal(|mut strip| {
                strip.cell(|ui| {
                    ui.label("Active accounts:");
                    ScrollArea::vertical().show(ui, |ui| {
                        ui.vertical(|ui| {
                            let (profiles, cur_profile) = accounts.profiles();
                            for account in profiles {
                                if account == cur_profile {
                                    ui.colored_label(Color32::WHITE, account);
                                } else {
                                    ui.label(account);
                                }
                            }
                        })
                    });
                });
                strip.cell(|ui| {
                    if ui
                        .button(icon_font_plus_text(ui, "\u{f0e0}", "Login with email"))
                        .clicked()
                    {
                        path.route_query_only_single((
                            PROFILE_PAGE_QUERY.into(),
                            "login-email".to_string(),
                        ));
                    }
                    ui.button(icon_font_plus_text(ui, "\u{f1b7}", "Login with Steam"));
                    ui.add_space(20.0);
                    if ui
                        .button(icon_font_plus_text(ui, "\u{f0e0}", "Register with email"))
                        .clicked()
                    {
                        path.route_query_only_single((
                            PROFILE_PAGE_QUERY.into(),
                            "register-email".to_string(),
                        ));
                    }
                });
            });
    });
}
