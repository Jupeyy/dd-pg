use std::ops::Range;

use client_types::console::ConsoleEntry;
use command_parser::parser::{self, CommandParseResult, CommandType, CommandsTyped, Syn};
use config::{
    config::ConfigEngine,
    traits::{ConfigFromStrErr, ConfigInterface},
};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use game_config::config::ConfigGame;

pub fn find_modifiers(in_str: &str) -> (String, Vec<String>) {
    let mut modifiers = Vec::new();
    let mut cur_modifier = String::new();
    let mut str_without_modifiers = String::new();
    let mut brackets = 0;
    for c in in_str.chars() {
        if c == '[' {
            brackets += 1;
        } else if c == ']' {
            brackets -= 1;

            if brackets == 0 {
                modifiers.push(cur_modifier);
                cur_modifier = String::new();
            }
        } else if brackets == 0 {
            str_without_modifiers.push(c);
        } else {
            cur_modifier.push(c);
        }
    }

    (str_without_modifiers, modifiers)
}

pub fn find_matches(
    cmds: &CommandsTyped,
    cursor_pos: usize,
    entries: &[ConsoleEntry],
    msg: &str,
) -> Vec<(usize, (i64, Vec<usize>))> {
    // if a cmd was found that wasn't finished, then suggest
    let Some(msg) = cmds.iter().rev().find_map(|cmd| {
        if let (
            CommandType::Partial(CommandParseResult::InvalidCommandIdent(range)),
            Some((cursor_byte_off, _)),
        ) = (cmd, msg.char_indices().nth(cursor_pos.saturating_sub(1)))
        {
            range
                .contains(&cursor_byte_off)
                .then_some(msg[range.clone()].to_string())
        } else {
            None
        }
    }) else {
        return Vec::new();
    };

    let matcher = SkimMatcherV2::default();
    let (console_inp_without_modifiers, modifiers) = find_modifiers(msg.trim());

    let mut found_entries: Vec<(usize, i64, (i64, Vec<usize>))> = entries
        .iter()
        .enumerate()
        .map(|(index, e)| match e {
            ConsoleEntry::Var(v) => {
                let max_modifiers =
                    v.full_name.matches("$KEY$").count() + v.full_name.matches("$INDEX$").count();
                (
                    index,
                    v.full_name.len() as i64,
                    if modifiers.len() <= max_modifiers {
                        matcher.fuzzy_indices(&v.full_name, &console_inp_without_modifiers)
                    } else {
                        None
                    },
                )
            }
            ConsoleEntry::Cmd(c) => (
                index,
                c.name.len() as i64,
                matcher.fuzzy_indices(&c.name, &console_inp_without_modifiers),
            ),
        })
        .filter(|(_, _, m)| m.is_some())
        .map(|(index, len, m)| (index, len, m.unwrap()))
        .collect();

    // not the cleanest way to respect the length in a score sorting, but dunno.
    found_entries.sort_by(|(_, len_a, (score_a, _)), (_, len_b, (score_b, _))| {
        (*score_b * u16::MAX as i64 - *len_b).cmp(&(*score_a * u16::MAX as i64 - *len_a))
    });

    found_entries
        .into_iter()
        .map(|(index, _, fuz)| (index, fuz))
        .collect()
}

pub fn find_matches_old(entries: &[ConsoleEntry], msg: &str) -> Vec<(usize, (i64, Vec<usize>))> {
    let matcher = SkimMatcherV2::default();
    let (console_inp_without_modifiers, modifiers) = find_modifiers(msg.trim());

    let mut found_entries: Vec<(usize, (i64, Vec<usize>))> = entries
        .iter()
        .enumerate()
        .map(|(index, e)| match e {
            ConsoleEntry::Var(v) => {
                let max_modifiers =
                    v.full_name.matches("$KEY$").count() + v.full_name.matches("$INDEX$").count();
                (
                    index,
                    if modifiers.len() <= max_modifiers {
                        matcher.fuzzy_indices(&v.full_name, &console_inp_without_modifiers)
                    } else {
                        None
                    },
                )
            }
            ConsoleEntry::Cmd(c) => (
                index,
                matcher.fuzzy_indices(&c.name, &console_inp_without_modifiers),
            ),
        })
        .filter(|(_, m)| m.is_some())
        .map(|(index, m)| (index, m.unwrap()))
        .collect();
    found_entries.sort_by(|(_, (score_a, _)), (_, (score_b, _))| score_b.cmp(score_a));
    found_entries
}

pub fn syn_vec_to_config_val(args: &[(Syn, Range<usize>)]) -> Option<String> {
    args.first().map(|(arg, _)| match arg {
        parser::Syn::Command(cmd) => cmd.cmd_text.clone(),
        parser::Syn::Commands(cmds) => cmds
            .first()
            .map(|cmd| cmd.cmd_text.clone())
            .unwrap_or_default(),
        parser::Syn::Text(text) => text.clone(),
        parser::Syn::Number(num) => num.clone(),
        parser::Syn::JsonObjectLike(obj) => obj.clone(),
        parser::Syn::JsonArrayLike(obj) => obj.clone(),
    })
}

pub fn try_apply_config_val(
    cmd_text: &str,
    args: &[(Syn, Range<usize>)],
    config_engine: &mut ConfigEngine,
    config_game: &mut ConfigGame,
) -> anyhow::Result<String, String> {
    let set_val = syn_vec_to_config_val(args);

    config_engine
        .try_set_from_str(cmd_text.to_owned(), None, set_val.clone(), None, 0)
        .or_else(|err| {
            config_game
                .try_set_from_str(cmd_text.to_owned(), None, set_val.clone(), None, 0)
                .map_err(|err_game| {
                    let mut was_fatal = false;
                    let mut msgs: String = Default::default();
                    match err {
                        ConfigFromStrErr::PathErr(_) => {
                            msgs.push_str(&format!("Parsing error: {}\n", err,));
                        }
                        ConfigFromStrErr::FatalErr(_) => {
                            was_fatal = true;
                        }
                    }
                    match err_game {
                        ConfigFromStrErr::PathErr(_) => {
                            msgs.push_str(&format!("Parsing error: {}\n", err_game,));
                            was_fatal = false;
                        }
                        ConfigFromStrErr::FatalErr(_) => {}
                    }
                    if was_fatal {
                        msgs.push_str(&format!("Parsing errors: {}, {}\n", err, err_game,));
                    }
                    msgs
                })
        })
}

pub fn run_command(
    cmd: &CommandType,
    entries: &[ConsoleEntry],
    config_engine: &mut ConfigEngine,
    config_game: &mut ConfigGame,
    msgs: &mut String,
) {
    if let Some(entry_cmd) = entries
        .iter()
        .filter_map(|e| match e {
            client_types::console::ConsoleEntry::Var(_) => None,
            client_types::console::ConsoleEntry::Cmd(c) => Some(c),
        })
        .find(|c| {
            if let CommandType::Full(cmd) = cmd {
                cmd.ident == c.name
            } else {
                false
            }
        })
    {
        let cmd = cmd.unwrap_ref_full();
        if let Err(err) = (entry_cmd.cmd)(config_engine, config_game, &cmd.args) {
            msgs.push_str(&format!("Parsing error: {}\n", err));
        }
    } else {
        let Some((args, cmd_text)) = (match cmd {
            CommandType::Full(cmd) => Some((cmd.args.clone(), &cmd.cmd_text)),
            CommandType::Partial(cmd) => {
                if let CommandParseResult::InvalidArg { partial_cmd, .. } = cmd {
                    Some((Vec::new(), &partial_cmd.cmd_text))
                } else {
                    None
                }
            }
        }) else {
            return;
        };

        let set_val = syn_vec_to_config_val(&args);

        match try_apply_config_val(cmd_text, &args, config_engine, config_game) {
            Ok(cur_val) => {
                if set_val.is_some() {
                    msgs.push_str(&format!(
                        "Updated value for \"{}\": {}\n",
                        cmd_text, cur_val
                    ));
                } else {
                    msgs.push_str(&format!(
                        "Current value for \"{}\": {}\n",
                        cmd_text, cur_val
                    ));
                }
            }
            Err(err) => {
                msgs.push_str(&err);
            }
        }
    }
}

pub fn run_commands(
    cmds: &CommandsTyped,
    entries: &[ConsoleEntry],
    config_engine: &mut ConfigEngine,
    config_game: &mut ConfigGame,
    msgs: &mut String,
) {
    for cmd in cmds {
        run_command(cmd, entries, config_engine, config_game, msgs);
    }
}
