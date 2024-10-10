use std::ops::Range;

use client_types::console::{
    entries_to_parser, ConsoleEntry, ConsoleEntryCmd, ConsoleEntryVariable,
};
use command_parser::parser::{parse, CommandParseResult, CommandType, CommandsTyped};
use egui::{
    text::{CCursor, LayoutJob},
    text_selection::CCursorRange,
    Color32, FontId, Id, Layout, RichText, TextFormat,
};

use ui_base::types::{UiRenderPipe, UiState};

use super::{
    user_data::UserData,
    utils::{find_matches, run_commands},
};

/// console input
pub fn render(
    ui: &mut egui::Ui,
    pipe: &mut UiRenderPipe<UserData>,
    ui_state: &mut UiState,
    has_text_selection: bool,
    cmds: &CommandsTyped,
) {
    let mouse_is_down = ui.input(|i| i.any_touches() || i.pointer.any_down());

    let msg_before_inp = pipe.user_data.msg.clone();
    let cursor_before_inp = *pipe.user_data.cursor;

    ui.style_mut().spacing.item_spacing.x = 0.0;
    ui.horizontal(|ui| {
        ui.add_space(5.0);
        ui.label(RichText::new(">").font(FontId::monospace(12.0)));
        ui.with_layout(
            Layout::left_to_right(egui::Align::Max).with_main_justify(true),
            |ui| {
                let inp_id = Id::new("console-input");

                let mut layouter = |ui: &egui::Ui, string: &str, _wrap_width: f32| {
                    let cmd = cmds.iter();
                    let mut layout_job = LayoutJob::default();
                    let mut last_range = 0;
                    let len = string.len();
                    let get_range = |last_range: usize, range: &Range<usize>| {
                        let start_range = range.start.min(len).max(last_range);
                        start_range..range.end.min(len).max(last_range)
                    };
                    fn text_fmt_base() -> TextFormat {
                        TextFormat {
                            valign: egui::Align::Max,
                            font_id: FontId::monospace(12.0),
                            color: Color32::WHITE,
                            ..Default::default()
                        }
                    }
                    let colorize_semicolons = |layout_job: &mut LayoutJob, range: Range<usize>| {
                        let s = &string[range];
                        let splits = || s.split(";");
                        let split_count = splits().count();
                        if split_count > 1 {
                            for (index, split) in splits().enumerate() {
                                layout_job.append(split, 0.0, text_fmt_base());

                                if index + 1 != split_count {
                                    layout_job.append(";", 0.0, {
                                        let mut fmt = text_fmt_base();
                                        fmt.color = Color32::LIGHT_RED;
                                        fmt
                                    });
                                }
                            }
                        } else {
                            layout_job.append(s, 0.0, text_fmt_base());
                        }
                    };
                    for cmd in cmd {
                        match cmd {
                            CommandType::Full(cmd) => {
                                let range = get_range(last_range, &cmd.cmd_range);
                                colorize_semicolons(&mut layout_job, last_range..range.start);
                                layout_job.append(&string[range.start..range.end], 0.0, {
                                    let mut fmt = text_fmt_base();
                                    fmt.color = Color32::GOLD;
                                    fmt
                                });
                                last_range = range.end;
                            }
                            CommandType::Partial(cmd) => {
                                let (range, color, err) = if let CommandParseResult::InvalidArg {
                                    partial_cmd,
                                    range,
                                    ..
                                } = cmd
                                {
                                    (
                                        get_range(last_range, &partial_cmd.cmd_range),
                                        Color32::GOLD,
                                        Some((range.clone(), Color32::LIGHT_GRAY)),
                                    )
                                } else {
                                    (
                                        get_range(last_range, cmd.range()),
                                        Color32::LIGHT_GRAY,
                                        None,
                                    )
                                };
                                colorize_semicolons(&mut layout_job, last_range..range.start);
                                layout_job.append(&string[range.start..range.end], 0.0, {
                                    let mut fmt = text_fmt_base();
                                    fmt.color = color;
                                    fmt
                                });
                                last_range = range.end;
                                if let Some((range, color)) = err {
                                    let range = get_range(last_range, &range);
                                    colorize_semicolons(&mut layout_job, last_range..range.start);
                                    layout_job.append(&string[range.start..range.end], 0.0, {
                                        let mut fmt = text_fmt_base();
                                        fmt.color = color;
                                        fmt
                                    });
                                    last_range = range.end;
                                }
                            }
                        }
                    }
                    colorize_semicolons(&mut layout_job, last_range..string.len());
                    ui.fonts(|f| f.layout_job(layout_job))
                };
                let mut label = egui::TextEdit::singleline(pipe.user_data.msg)
                    .font(FontId::monospace(12.0))
                    .id(inp_id)
                    .layouter(&mut layouter)
                    .frame(false)
                    .show(ui);
                *pipe.user_data.cursor = label
                    .state
                    .cursor
                    .char_range()
                    .map(|cursor| cursor.primary.index)
                    .unwrap_or_default();
                let (enter, tab, space, modifiers) = ui.input(|i| {
                    (
                        i.key_pressed(egui::Key::Enter),
                        i.key_pressed(egui::Key::Tab),
                        i.key_pressed(egui::Key::Space),
                        i.modifiers,
                    )
                });

                if label.response.lost_focus() {
                    if enter && !pipe.user_data.msg.is_empty() {
                        // check if an entry was selected, execute that in that case
                        if let Some(index) = *pipe.user_data.select_index {
                            let entries = find_matches(
                                cmds,
                                cursor_before_inp,
                                pipe.user_data.entries,
                                &msg_before_inp,
                            );
                            let cur_entry = entries.iter().find(|(e, _)| *e == index);
                            if let Some((e, _)) = cur_entry {
                                match &pipe.user_data.entries[*e] {
                                    ConsoleEntry::Var(ConsoleEntryVariable {
                                        full_name: name,
                                        ..
                                    })
                                    | ConsoleEntry::Cmd(ConsoleEntryCmd { name, .. }) => {
                                        *pipe.user_data.msg = name.clone();
                                    }
                                }
                            }
                            *pipe.user_data.select_index = None;
                        }

                        let cmds = parse(
                            &*pipe.user_data.msg,
                            &entries_to_parser(pipe.user_data.entries),
                        );
                        run_commands(
                            &cmds,
                            pipe.user_data.entries,
                            &mut pipe.user_data.config.engine,
                            &mut pipe.user_data.config.game,
                            pipe.user_data.msgs,
                        );
                        pipe.user_data.msg.clear();
                    } else if tab {
                        // nothing to do here
                    } else if label.response.changed() {
                        // reset entry index
                        *pipe.user_data.select_index = None;
                    }
                } else if space && pipe.user_data.select_index.is_some() {
                    let index = pipe.user_data.select_index.unwrap();
                    let entries = find_matches(
                        cmds,
                        cursor_before_inp,
                        pipe.user_data.entries,
                        &msg_before_inp,
                    );
                    let cur_entry = entries.iter().find(|(e, _)| *e == index);
                    let mut cursor_next = None;
                    if let Some((e, _)) = cur_entry {
                        match &pipe.user_data.entries[*e] {
                            ConsoleEntry::Var(ConsoleEntryVariable {
                                full_name: name, ..
                            })
                            | ConsoleEntry::Cmd(ConsoleEntryCmd { name, .. }) => {
                                let err =
                                    cmds.iter().rev().find_map(|cmd| {
                                        if let (
                                            CommandType::Partial(cmd),
                                            Some((cursor_byte_off, _)),
                                        ) = (
                                            cmd,
                                            pipe.user_data
                                                .msg
                                                .char_indices()
                                                .nth(cursor_before_inp.saturating_sub(1)),
                                        ) {
                                            cmd.range().contains(&cursor_byte_off).then_some(cmd)
                                        } else {
                                            None
                                        }
                                    });

                                if let Some(CommandParseResult::InvalidCommandIdent(range)) = err {
                                    let name_len = name.chars().count();
                                    let cur_cursor_start = pipe
                                        .user_data
                                        .msg
                                        .char_indices()
                                        .enumerate()
                                        .find_map(|(index, (off, _))| {
                                            (off == range.start).then_some(index)
                                        })
                                        .unwrap_or_default();
                                    *pipe.user_data.msg = msg_before_inp;
                                    pipe.user_data
                                        .msg
                                        .replace_range(range.clone(), &format!("{name} "));
                                    cursor_next = Some((cur_cursor_start + name_len) + 1);
                                }
                            }
                        }
                    }

                    *pipe.user_data.select_index = None;
                    label.state.cursor.set_char_range(cursor_next.map(|index| {
                        CCursorRange::one(CCursor {
                            index,
                            ..Default::default()
                        })
                    }));
                    label.state.store(ui.ctx(), inp_id);
                } else if (!mouse_is_down && !has_text_selection) || ui_state.hint_had_input {
                    label.response.request_focus();
                }
                if tab {
                    // select next entry
                    let entries = find_matches(
                        cmds,
                        *pipe.user_data.cursor,
                        pipe.user_data.entries,
                        pipe.user_data.msg,
                    );
                    let it: Box<dyn Iterator<Item = _>> = if !modifiers.shift {
                        Box::new(entries.iter())
                    } else {
                        Box::new(entries.iter().rev())
                    };

                    let mut cur_entry = it
                        .skip_while(|(e, _)| {
                            if let Some(i) = pipe.user_data.select_index {
                                *e != *i
                            } else {
                                true
                            }
                        })
                        .peekable();
                    // skip the found element
                    cur_entry.next();
                    if let Some((cur_entry, _)) = cur_entry.next() {
                        *pipe.user_data.select_index = Some(*cur_entry);
                    } else {
                        // try select first entry
                        let mut it: Box<dyn Iterator<Item = _>> = if !modifiers.shift {
                            Box::new(entries.iter())
                        } else {
                            Box::new(entries.iter().rev())
                        };
                        if let Some((cur_entry, _)) = it.next() {
                            *pipe.user_data.select_index = Some(*cur_entry);
                        }
                    }
                }
            },
        );
    });
}
