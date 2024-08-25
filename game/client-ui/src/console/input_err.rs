use command_parser::parser::{CommandType, CommandsTyped};
use egui::{Color32, FontId, RichText};
use ui_base::types::{UiRenderPipe, UiState};

use super::user_data::UserData;

/// console input err
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    has_text_selection: bool,
    cmds: &CommandsTyped,
) {
    let err = cmds.iter().rev().find_map(|cmd| {
        if let CommandType::Partial(cmd) = cmd {
            Some(cmd)
        } else {
            None
        }
    });
    if let Some(err_range) = err.as_ref().map(|err| err.range()) {
        ui.horizontal_top(|ui| {
            ui.add_space(9.0);
            // two whitespaces for `>` console prefix
            let mut tilde = " ".to_string();
            for _ in 0..err_range.start {
                tilde.push(' ');
            }
            for _ in err_range.start..err_range.end {
                tilde.push('~');
            }
            ui.label(
                RichText::new(tilde)
                    .font(FontId::monospace(12.0))
                    .color(Color32::RED),
            );
        });
    }
    if let Some(err) = err {
        ui.label(err.to_string());
    }
}
