use config::config::ConfigEngine;
use egui::ComboBox;
use game_config::config::ConfigGame;
use ui_base::utils::icon_font_plus_text;

pub fn profile_selector(
    ui: &mut egui::Ui,
    id: &str,
    config: &ConfigGame,
    config_engine: &mut ConfigEngine,
) -> u64 {
    let path = &mut config_engine.ui.path;
    let profile_index = path
        .query
        .get("selected-profile")
        .and_then(|profile| profile.parse::<u64>().ok())
        .unwrap_or_default();

    ui.horizontal(|ui| {
        ui.label("Profile");
        ComboBox::new(id, "")
            .selected_text(if (profile_index as usize) < config.players.len() {
                &config.players[profile_index as usize].name
            } else {
                ""
            })
            .show_ui(ui, |ui| {
                for (index, p) in config.players.iter().enumerate() {
                    if ui
                        .button(icon_font_plus_text(
                            ui,
                            if index == config.profiles.main as usize {
                                "\u{f007}"
                            } else if index == config.profiles.dummy.index as usize {
                                "\u{f544}"
                            } else {
                                ""
                            },
                            &p.name,
                        ))
                        .clicked()
                    {
                        // Update selected profile
                        *path
                            .query
                            .entry("selected-profile".to_string())
                            .or_default() = (index as u64).to_string();
                    }
                }
            });
        if ui.button("Player's profile").clicked() {
            *path
                .query
                .entry("selected-profile".to_string())
                .or_default() = (config
                .profiles
                .main
                .clamp(0, config.players.len().saturating_sub(1) as u64))
            .to_string();
        }
        if ui.button("Dummy's profile").clicked() {
            *path
                .query
                .entry("selected-profile".to_string())
                .or_default() = (config
                .profiles
                .dummy
                .index
                .clamp(0, config.players.len().saturating_sub(1) as u64))
            .to_string();
        }
    });
    profile_index
}
