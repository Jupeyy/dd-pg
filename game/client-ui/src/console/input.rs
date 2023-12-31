use egui::Layout;
use graphics::graphics::Graphics;
use ui_base::types::{UIPipe, UIState};

use super::{
    user_data::UserData,
    utils::{find_matches, run_command},
};

/// console input
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UIPipe<UserData>,
    ui_state: &mut UIState,
    _graphics: &mut Graphics,
    has_text_selection: bool,
) {
    let mouse_is_down = ui.input(|i| i.any_touches() || i.pointer.any_down());
    ui.horizontal(|ui| {
        ui.add_space(5.0);
        ui.label(">");
        ui.with_layout(
            Layout::left_to_right(egui::Align::Center).with_main_justify(true),
            |ui| {
                let label = egui::TextEdit::singleline(pipe.user_data.msg)
                    .id_source("console-input")
                    .show(ui);
                if label.response.lost_focus() {
                    let (enter, tab) = ui.input(|i| {
                        (
                            i.key_pressed(egui::Key::Enter),
                            i.key_pressed(egui::Key::Tab),
                        )
                    });
                    if enter && !pipe.user_data.msg.is_empty() {
                        run_command(
                            pipe.user_data.msg,
                            pipe.user_data.entries,
                            pipe.config,
                            pipe.user_data.config_game,
                            pipe.user_data.msgs,
                        );
                        pipe.user_data.msg.clear();
                        // TODO:
                    } else if tab {
                        // select next entry
                        let entries = find_matches(pipe.user_data.entries, &pipe.user_data.msg);
                        let mut cur_entry = entries
                            .iter()
                            .skip_while(|(e, _)| {
                                if let Some(i) = pipe.user_data.select_index {
                                    *e != *i
                                } else {
                                    true
                                }
                            })
                            .peekable();
                        if let Some((cur_entry, _)) = cur_entry.next() {
                            *pipe.user_data.select_index = Some(*cur_entry);
                        } else {
                            // try select first entry
                            if let Some((cur_entry, _)) = entries.iter().next() {
                                *pipe.user_data.select_index = Some(*cur_entry);
                            }
                        }
                    } else if label.response.changed() {
                        // reset entry index
                        *pipe.user_data.select_index = None;
                    }
                } else {
                    if (!mouse_is_down && !has_text_selection) || ui_state.hint_had_input {
                        label.response.request_focus();
                    }
                }
            },
        );
    });
}
