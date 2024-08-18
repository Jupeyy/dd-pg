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
                            let mut profiles: Vec<_> = profiles.into_iter().collect();
                            profiles.sort_by_key(|(key, _)| key.clone());
                            for (key, account) in profiles {
                                if key.as_str() == cur_profile.as_str() {
                                    ui.colored_label(Color32::WHITE, &account.name);
                                } else {
                                    ui.label(&account.name);
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
                            "login-email-token".to_string(),
                        ));
                    }
                    if ui
                        .button(icon_font_plus_text(ui, "\u{f1b7}", "Login with Steam"))
                        .clicked()
                    {
                        path.route_query_only_single((
                            PROFILE_PAGE_QUERY.into(),
                            "login-steam".to_string(),
                        ));
                    }
                });
            });
    });
}
