use client_types::console::ConsoleEntry;
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

pub fn find_matches(entries: &Vec<ConsoleEntry>, msg: &String) -> Vec<(usize, (i64, Vec<usize>))> {
    let matcher = SkimMatcherV2::default();
    let (console_inp_without_modifiers, modifiers) = find_modifiers(&msg);

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

pub fn run_command(
    msg: &String,
    entries: &Vec<ConsoleEntry>,
    config_engine: &mut ConfigEngine,
    config_game: &mut ConfigGame,
    msgs: &mut String,
) {
    let mut splits = msg.splitn(2, " ");
    let (path, val) = (splits.next(), splits.next());
    let set_val = {
        let val = val.map(|s| s.to_string()).unwrap_or_default();

        if val.is_empty() {
            None
        } else {
            Some(val)
        }
    };
    let path_str = path.map(|p| p.to_string()).unwrap_or_default();
    let val_str = val.map(|p| p.to_string()).unwrap_or_default();
    if let Some(cmd) = entries
        .iter()
        .filter_map(|e| match e {
            client_types::console::ConsoleEntry::Var(_) => None,
            client_types::console::ConsoleEntry::Cmd(c) => Some(c),
        })
        .find(|c| path_str.starts_with(&c.name))
    {
        if let Err(err) = (cmd.cmd)(config_engine, config_game, val_str) {
            msgs.push_str(&format!("Parsing error: {}\n", err.to_string()));
        }
    } else {
        if let Some(cur_val) = config_engine
            .try_set_from_str(path_str.clone(), None, set_val.clone(), None, 0)
            .map(|v| Some(v))
            .unwrap_or_else(|err| {
                config_game
                    .try_set_from_str(path_str, None, set_val.clone(), None, 0)
                    .map(|v| Some(v))
                    .unwrap_or_else(|err_game| {
                        let mut was_fatal = false;
                        match err {
                            ConfigFromStrErr::PathErr(_) => {
                                msgs.push_str(&format!("Parsing error: {}\n", err.to_string(),));
                            }
                            ConfigFromStrErr::FatalErr(_) => {
                                was_fatal = true;
                            }
                        }
                        match err_game {
                            ConfigFromStrErr::PathErr(_) => {
                                msgs.push_str(&format!(
                                    "Parsing error: {}\n",
                                    err_game.to_string(),
                                ));
                                was_fatal = false;
                            }
                            ConfigFromStrErr::FatalErr(_) => {}
                        }
                        if was_fatal {
                            msgs.push_str(&format!(
                                "Parsing errors: {}, {}\n",
                                err.to_string(),
                                err_game.to_string(),
                            ));
                        }
                        None
                    })
            })
        {
            if set_val.is_some() {
                msgs.push_str(&format!("Updated value: {}\n", cur_val));
            } else {
                msgs.push_str(&format!("Current value: {}\n", cur_val));
            }
        }
    }
}
