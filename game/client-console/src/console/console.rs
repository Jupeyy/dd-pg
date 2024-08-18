use std::rc::Rc;

use base::system::{self, SystemTimeInterface};
use binds::binds::{
    bind_to_str, gen_local_player_action_hash_map, gen_local_player_action_hash_map_rev,
    syn_to_bind,
};
use client_render::generic_ui_renderer;
use client_types::console::{
    entries_to_parser, ConsoleEntry, ConsoleEntryCmd, ConsoleEntryVariable,
};
use client_ui::console::{page::ConsoleUi, user_data::UserData, utils::syn_vec_to_config_val};
use command_parser::parser::{parse, CommandArg, CommandArgType};
use config::{
    config::ConfigEngine,
    traits::{ConfigFromStrFlags, ConfigInterface, ConfigValue},
};
use egui::Color32;
use game_config::config::{Config, ConfigGame};
use graphics::graphics::graphics::Graphics;
use hiarc::{hiarc_safer_rc_refcell, Hiarc};
use native::native::NativeImpl;
use ui_base::{
    types::UiRenderPipe,
    ui::{UiContainer, UiCreator},
};

#[derive(Debug, Hiarc)]
pub enum ConsoleEvent {
    Quit,
}

pub struct ConsoleRenderPipe<'a> {
    pub graphics: &'a mut Graphics,
    pub sys: &'a system::System,
    pub config: &'a mut Config,
    pub msgs: &'a mut String,
}

#[hiarc_safer_rc_refcell]
#[derive(Debug, Default, Hiarc)]
pub struct ConsoleEvents {
    events: Vec<ConsoleEvent>,
}

#[hiarc_safer_rc_refcell]
impl ConsoleEvents {
    pub fn push(&mut self, ev: ConsoleEvent) {
        self.events.push(ev)
    }

    pub fn take(&mut self) -> Vec<ConsoleEvent> {
        std::mem::take(&mut self.events)
    }
}

pub struct ConsoleRender {
    pub ui: UiContainer,
    pub entries: Vec<ConsoleEntry>,
    pub text: String,
    pub cursor: usize,
    pub selected_index: Option<usize>,
    pub console_ui: ConsoleUi,

    console_events: ConsoleEvents,
}

impl ConsoleRender {
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

    fn register_commands(console_events: ConsoleEvents, list: &mut Vec<ConsoleEntry>) {
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
                console_events.push(ConsoleEvent::Quit);
                Ok(())
            }),
            args: vec![],
        }));
    }

    pub fn new(
        config_game: &mut ConfigGame,
        config_engine: &mut ConfigEngine,
        native: &mut dyn NativeImpl,
        creator: &UiCreator,
    ) -> Self {
        let mut ui = UiContainer::new(None, creator);
        ui.set_main_panel_color(&Color32::TRANSPARENT);

        let console_events: ConsoleEvents = Default::default();
        let mut entries: Vec<ConsoleEntry> = Vec::new();
        let val = ConfigEngine::conf_value();
        Self::parse_conf_values_as_str_list("".to_string(), &mut entries, val, false);
        let val = ConfigGame::conf_value();
        Self::parse_conf_values_as_str_list("".to_string(), &mut entries, val, false);
        Self::register_commands(console_events.clone(), &mut entries);

        let cmd = native.start_arguments().join(" ");
        if !cmd.is_empty() {
            let cmds = parse(&cmd, &entries_to_parser(&entries));
            client_ui::console::utils::run_commands(
                &cmds,
                &entries,
                config_engine,
                config_game,
                &mut String::new(),
            );
        }

        Self {
            ui,
            entries,
            text: Default::default(),
            selected_index: None,
            cursor: 0,
            console_ui: ConsoleUi {},
            console_events,
        }
    }

    #[must_use]
    pub fn render(
        &mut self,
        inp: egui::RawInput,
        pipe: &mut ConsoleRenderPipe,
    ) -> (Vec<ConsoleEvent>, egui::PlatformOutput) {
        let mut user_data = UserData {
            entries: &self.entries,
            msgs: pipe.msgs,
            msg: &mut self.text,
            cursor: &mut self.cursor,
            select_index: &mut self.selected_index,
            config: pipe.config,
        };
        let mut ui_pipe = UiRenderPipe::new(pipe.sys.time_get_nanoseconds(), &mut user_data);

        let res = generic_ui_renderer::render(
            &pipe.graphics.backend_handle,
            &pipe.graphics.texture_handle,
            &pipe.graphics.stream_handle,
            &pipe.graphics.canvas_handle,
            &mut self.ui,
            &mut self.console_ui,
            &mut ui_pipe,
            inp.clone(),
            inp,
        );

        (self.get_events(), res)
    }

    #[must_use]
    pub fn get_events(&self) -> Vec<ConsoleEvent> {
        self.console_events.take()
    }
}
