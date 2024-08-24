use std::rc::Rc;

use binds::binds::{
    bind_to_str, gen_local_player_action_hash_map, gen_local_player_action_hash_map_rev,
    syn_to_bind,
};
use client_types::console::{ConsoleEntry, ConsoleEntryCmd, ConsoleEntryVariable};
use client_ui::console::utils::syn_vec_to_config_val;
use command_parser::parser::{CommandArg, CommandArgType};
use config::{
    config::ConfigEngine,
    traits::{ConfigFromStrFlags, ConfigInterface, ConfigValue},
};
use egui::Color32;
use game_config::config::ConfigGame;
use hiarc::{hiarc_safer_rc_refcell, Hiarc};
use ui_base::ui::UiCreator;

use super::console::ConsoleRender;

#[derive(Debug, Hiarc)]
pub enum LocalConsoleEvent {
    Quit,
}

#[hiarc_safer_rc_refcell]
#[derive(Debug, Default, Hiarc)]
pub struct LocalConsoleEvents {
    events: Vec<LocalConsoleEvent>,
}

#[hiarc_safer_rc_refcell]
impl LocalConsoleEvents {
    pub fn push(&mut self, ev: LocalConsoleEvent) {
        self.events.push(ev)
    }
}

#[hiarc_safer_rc_refcell]
impl super::console::ConsoleEvents<LocalConsoleEvent> for LocalConsoleEvents {
    #[hiarc_trait_is_immutable_self]
    fn take(&mut self) -> Vec<LocalConsoleEvent> {
        std::mem::take(&mut self.events)
    }
}

pub type LocalConsole = ConsoleRender<LocalConsoleEvent, ()>;

#[derive(Debug, Default)]
pub struct LocalConsoleBuilder {}

impl LocalConsoleBuilder {
    fn struct_name(val: &ConfigValue) -> &str {
        if let ConfigValue::Struct { name, .. } = val {
            name
        } else {
            ""
        }
    }

    fn parse_conf_value_usage(val: &ConfigValue) -> String {
        match val {
            ConfigValue::Struct { .. } => "unsupported".to_string(),
            ConfigValue::Int { min, max } => {
                format!("int [{min}..{max}]")
            }
            ConfigValue::Float { min, max } => {
                format!("float [{min},{max}]")
            }
            ConfigValue::String {
                min_length,
                max_length,
            } => {
                format!("string, length range [{min_length}..{max_length}]")
            }
            ConfigValue::StringOfList { allowed_values } => {
                format!("string in [{}]", allowed_values.join(", "))
            }
            ConfigValue::Array { val_ty, .. } => {
                format!(
                    "array of [{}] (access/set: [numberic index], remove: `pop`-cmd, \
                    insert: `push`-cmd, assign whole array by JSON)",
                    Self::struct_name(val_ty)
                )
            }
            ConfigValue::JsonLikeRecord { val_ty } => {
                format!(
                    "JSON-like record (access/insert/set: [alphabetic index], \
                    rem: `rem`-cmd + [alphabetic index], assign whole record by JSON) \
                    {{ \"index\": \"{}\" }}",
                    Self::parse_conf_value_usage(val_ty)
                )
            }
        }
    }

    fn parse_conf_values_as_str_list(
        cur_path: String,
        list: &mut Vec<ConsoleEntry>,
        val: ConfigValue,
        is_alias: bool,
    ) {
        let usage = Self::parse_conf_value_usage(&val);
        match val {
            ConfigValue::Struct {
                attributes,
                aliases,
                ..
            } => {
                list.push(ConsoleEntry::Var(ConsoleEntryVariable {
                    full_name: cur_path.clone(),
                    usage,
                    args: vec![CommandArg {
                        expected_ty: CommandArgType::JsonObjectLike,
                    }],
                }));

                for attribute in attributes {
                    let mut new_path = cur_path.clone();
                    if !cur_path.is_empty() {
                        new_path.push('.');
                    }

                    let path_without_name = new_path.clone();
                    new_path.push_str(&attribute.name);
                    Self::parse_conf_values_as_str_list(
                        new_path,
                        list,
                        attribute.val.clone(),
                        false,
                    );

                    // check if attribute has potential alias
                    for (from, to) in &aliases {
                        if to
                            .to_lowercase()
                            .starts_with(&attribute.name.to_lowercase())
                        {
                            let (rest, _) = client_ui::console::utils::find_modifiers(to.as_str());
                            // quickly recheck if the attribute is really correct
                            if rest.to_lowercase() == attribute.name.to_lowercase() {
                                let mut path = path_without_name.clone();
                                path.push_str(from.as_str());
                                Self::parse_conf_values_as_str_list(
                                    path,
                                    list,
                                    attribute.val.clone(),
                                    true,
                                );
                            }
                        }
                    }
                }
            }
            ConfigValue::JsonLikeRecord { ref val_ty } | ConfigValue::Array { ref val_ty, .. } => {
                let mut new_path = cur_path.clone();

                // push the object itself
                list.push(ConsoleEntry::Var(ConsoleEntryVariable {
                    full_name: cur_path,
                    usage,
                    args: vec![CommandArg {
                        expected_ty: if matches!(val, ConfigValue::JsonLikeRecord { .. }) {
                            CommandArgType::JsonObjectLike
                        } else {
                            CommandArgType::JsonArrayLike
                        },
                    }],
                }));

                // and object access/set/etc. of the types
                if !is_alias {
                    if let ConfigValue::JsonLikeRecord { .. } = val {
                        new_path.push_str("$KEY$");
                    } else {
                        new_path.push_str("$INDEX$");
                    }
                }
                Self::parse_conf_values_as_str_list(new_path, list, *val_ty.clone(), false);
            }
            ref conf_val => {
                list.push(ConsoleEntry::Var(ConsoleEntryVariable {
                    full_name: cur_path,
                    args: vec![CommandArg {
                        expected_ty: match conf_val {
                            ConfigValue::Float { .. } | ConfigValue::Int { .. } => {
                                CommandArgType::Number
                            }
                            _ => CommandArgType::Text,
                        },
                    }],
                    usage,
                }));
            }
        }
    }

    fn register_commands(console_events: LocalConsoleEvents, list: &mut Vec<ConsoleEntry>) {
        list.push(ConsoleEntry::Cmd(ConsoleEntryCmd {
            name: "push".into(),
            usage: "push <var>".into(),
            cmd: Rc::new(|config_engine, config_game, path| {
                let path = syn_vec_to_config_val(path).unwrap_or_default();
                if config_engine
                    .try_set_from_str(
                        path.clone(),
                        None,
                        None,
                        None,
                        ConfigFromStrFlags::Push as i32,
                    )
                    .is_err()
                    && config_game
                        .try_set_from_str(path, None, None, None, ConfigFromStrFlags::Push as i32)
                        .is_err()
                {
                    return Err(anyhow::anyhow!("No array variable with that name found"));
                }
                Ok(())
            }),
            args: vec![CommandArg {
                expected_ty: CommandArgType::CommandIdent,
            }],
        }));
        list.push(ConsoleEntry::Cmd(ConsoleEntryCmd {
            name: "pop".into(),
            usage: "pop <var>".into(),
            cmd: Rc::new(|config_engine, config_game, path| {
                let path = syn_vec_to_config_val(path).unwrap_or_default();
                if config_engine
                    .try_set_from_str(
                        path.clone(),
                        None,
                        None,
                        None,
                        ConfigFromStrFlags::Pop as i32,
                    )
                    .is_err()
                    && config_game
                        .try_set_from_str(path, None, None, None, ConfigFromStrFlags::Pop as i32)
                        .is_err()
                {
                    return Err(anyhow::anyhow!("No array variable with that name found"));
                }
                Ok(())
            }),
            args: vec![CommandArg {
                expected_ty: CommandArgType::CommandIdent,
            }],
        }));
        list.push(ConsoleEntry::Cmd(ConsoleEntryCmd {
            name: "rem".into(),
            usage: "rem <var>[key]".into(),
            cmd: Rc::new(|config_engine, config_game, path| {
                let path = syn_vec_to_config_val(path).unwrap_or_default();
                if config_engine
                    .try_set_from_str(
                        path.clone(),
                        None,
                        None,
                        None,
                        ConfigFromStrFlags::Rem as i32,
                    )
                    .is_err()
                    && config_game
                        .try_set_from_str(path, None, None, None, ConfigFromStrFlags::Rem as i32)
                        .is_err()
                {
                    return Err(anyhow::anyhow!("No record variable with that key found"));
                }
                Ok(())
            }),
            args: vec![CommandArg {
                expected_ty: CommandArgType::CommandIdent,
            }],
        }));
        let actions_map = gen_local_player_action_hash_map();
        let actions_map_rev = gen_local_player_action_hash_map_rev();

        for name in actions_map.keys() {
            list.push(ConsoleEntry::Cmd(ConsoleEntryCmd {
                name: name.to_string(),
                usage: format!("triggers a player action: {}", name),
                cmd: Rc::new(move |_config_engine, _config_game, _path| Ok(())),
                args: vec![],
            }));
        }

        list.push(ConsoleEntry::Cmd(ConsoleEntryCmd {
            name: "bind".into(),
            usage: "bind <keys> <commands>".into(),
            cmd: Rc::new(move |_config_engine, config_game, path| {
                let (keys, action) = syn_to_bind(path, &actions_map)?;
                config_game.players[0]
                    .binds
                    .push(bind_to_str(&keys, action, &actions_map_rev));
                Ok(())
            }),
            args: vec![
                CommandArg {
                    expected_ty: CommandArgType::TextFrom({
                        let mut res = vec![];
                        for i in 'a'..='z' {
                            res.push(i.to_string());
                        }
                        for i in '0'..='9' {
                            res.push(i.to_string());
                        }
                        for i in 0..35 {
                            res.push(format!("f{}", i + 1));
                        }

                        res.push("enter".to_string());
                        res.push("ctrl".to_string());
                        res.push("shift".to_string());
                        res.push("alt".to_string());
                        // TODO: add lot more
                        res
                    }),
                },
                CommandArg {
                    expected_ty: CommandArgType::Commands,
                },
            ],
        }));
        list.push(ConsoleEntry::Cmd(ConsoleEntryCmd {
            name: "quit".into(),
            usage: "quit the client".into(),
            cmd: Rc::new(move |_, _, _| {
                console_events.push(LocalConsoleEvent::Quit);
                Ok(())
            }),
            args: vec![],
        }));
    }

    pub fn build(creator: &UiCreator) -> LocalConsole {
        let console_events: LocalConsoleEvents = Default::default();
        let mut entries: Vec<ConsoleEntry> = Vec::new();
        let val = ConfigEngine::conf_value();
        Self::parse_conf_values_as_str_list("".to_string(), &mut entries, val, false);
        let val = ConfigGame::conf_value();
        Self::parse_conf_values_as_str_list("".to_string(), &mut entries, val, false);
        Self::register_commands(console_events.clone(), &mut entries);
        ConsoleRender::new(
            creator,
            entries,
            Box::new(console_events),
            Color32::from_rgba_unmultiplied(0, 0, 0, 150),
            (),
        )
    }
}
