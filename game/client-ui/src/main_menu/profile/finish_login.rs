use config::config::ConfigPath;
use egui::{ScrollArea, Spinner};

use crate::main_menu::user_data::ProfileTasks;

use super::{back_bar::back_bar, constants::PROFILE_PAGE_QUERY};

/// overview
pub fn render(ui: &mut egui::Ui, tasks: &mut ProfileTasks, path: &mut ConfigPath) {
    back_bar(ui, "Login by email", path);

    let loading = tasks.logins.iter().any(|task| !task.is_finished());
    if loading {
        ui.add(Spinner::new());
    } else {
        for task in tasks.logins.drain(..) {
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
            path.route_query_only_single((PROFILE_PAGE_QUERY.into(), "".to_string()));
        }
    }
}
