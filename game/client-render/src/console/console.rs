use std::{cell::RefCell, rc::Rc};

use base::system::{self, SystemTimeInterface};
use client_types::console::{ConsoleEntry, ConsoleEntryCmd, ConsoleEntryVariable};
use client_ui::console::{page::ConsoleUI, user_data::UserData};
use config::{
    config::ConfigEngine,
    traits::{ConfigFromStrFlags, ConfigInterface, ConfigValue},
};
use egui::Color32;
use egui_winit::State;
use game_config::config::ConfigGame;
use graphics::graphics::Graphics;

use native::native::NativeImpl;
use ui_base::{
    types::{UIFeedbackInterface, UINativePipe, UIPipe},
    ui::UI,
};
use ui_wasm_manager::{UIWinitWrapper, UIWinitWrapperDummyPipe, UIWinitWrapperPipe};

use crate::generic_ui_renderer;

pub enum ConsoleEvent {
    Quit,
}

pub struct ConsoleRenderPipe<'a> {
    pub graphics: &'a mut Graphics,
    pub sys: &'a system::System,
    pub config_engine: &'a mut ConfigEngine,
    pub config_game: &'a mut ConfigGame,
    pub msgs: &'a mut String,
    pub window: &'a winit::window::Window,
}

pub struct ClientStatsUIFeedbackDummy {}

impl UIFeedbackInterface for ClientStatsUIFeedbackDummy {}

pub struct ConsoleRender {
    pub ui: UI<UIWinitWrapper>,
    pub entries: Vec<ConsoleEntry>,
    pub text: String,
    pub selected_index: Option<usize>,
    pub console_ui: ConsoleUI,

    console_events: Rc<RefCell<Vec<ConsoleEvent>>>,
}

impl ConsoleRender {
    fn parse_conf_value_usage(val: ConfigValue) -> String {
        match val {
            ConfigValue::Struct { .. } => "unsupported".to_string(),
            ConfigValue::Int { min, max } => {
                "int [".to_string() + &min.to_string() + ".." + &max.to_string() + "]"
            }
            ConfigValue::Float { min, max } => {
                "float [".to_string() + &min.to_string() + "," + &max.to_string() + "]"
            }
            ConfigValue::String {
                min_length,
                max_length,
            } => {
                "string, length range [".to_string()
                    + &min_length.to_string()
                    + ".."
                    + &max_length.to_string()
                    + "]"
            }
            ConfigValue::StringOfList { allowed_values } => {
                "string in [".to_string() + &allowed_values.join(", ") + "]"
            }
            ConfigValue::Array { val_ty, .. } => {
                "array (access/set: [numberic index], remove: `pop`-cmd, insert: `push`-cmd, allows to assign whole array by JSON) of [".to_string()
                    + &Self::parse_conf_value_usage(*val_ty)
                    + "]"
            }
            ConfigValue::JSONRecord { val_ty } => {
                "JSON-like record (access/insert/set: [alphabetic index], rem: `rem`-cmd + [alphabetic index], allows to assign whole record by JSON) { \"index\": \"".to_string()
                    + &Self::parse_conf_value_usage(*val_ty)
                    + "\" }"
            }
        }
    }

    fn parse_conf_values_as_str_list(
        cur_path: String,
        list: &mut Vec<ConsoleEntry>,
        val: ConfigValue,
        is_alias: bool,
    ) {
        match val {
            ConfigValue::Struct {
                attributes,
                aliases,
            } => {
                for attribute in attributes {
                    let mut new_path = cur_path.clone();
                    if !cur_path.is_empty() {
                        new_path.push_str(".");
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
                            // quickly recheck if the attribut is really correct
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
            ConfigValue::JSONRecord { ref val_ty } | ConfigValue::Array { ref val_ty, .. } => {
                let mut new_path = cur_path.clone();

                // push the object itself
                list.push(ConsoleEntry::Var(ConsoleEntryVariable {
                    full_name: cur_path,
                    usage: Self::parse_conf_value_usage(val.clone()),
                }));

                // and object access/set/etc. of the types
                if !is_alias {
                    if let ConfigValue::JSONRecord { .. } = val {
                        new_path.push_str("$KEY$");
                    } else {
                        new_path.push_str("$INDEX$");
                    }
                }
                Self::parse_conf_values_as_str_list(new_path, list, *val_ty.clone(), false);
            }
            _ => {
                list.push(ConsoleEntry::Var(ConsoleEntryVariable {
                    full_name: cur_path,
                    usage: Self::parse_conf_value_usage(val),
                }));
            }
        }
    }

    fn register_commands(
        console_events: Rc<RefCell<Vec<ConsoleEvent>>>,
        list: &mut Vec<ConsoleEntry>,
    ) {
        list.push(ConsoleEntry::Cmd(ConsoleEntryCmd {
            name: "push".into(),
            usage: "push <var>".into(),
            cmd: Rc::new(|config_engine, config_game, path| {
                if config_engine
                    .try_set_from_str(
                        path.clone(),
                        None,
                        None,
                        None,
                        ConfigFromStrFlags::Push as i32,
                    )
                    .is_err()
                {
                    if config_game
                        .try_set_from_str(path, None, None, None, ConfigFromStrFlags::Push as i32)
                        .is_err()
                    {
                        return Err(anyhow::anyhow!("No array variable with that name found"));
                    }
                }
                Ok(())
            }),
        }));
        list.push(ConsoleEntry::Cmd(ConsoleEntryCmd {
            name: "pop".into(),
            usage: "pop <var>".into(),
            cmd: Rc::new(|config_engine, config_game, path| {
                if config_engine
                    .try_set_from_str(
                        path.clone(),
                        None,
                        None,
                        None,
                        ConfigFromStrFlags::Pop as i32,
                    )
                    .is_err()
                {
                    if config_game
                        .try_set_from_str(path, None, None, None, ConfigFromStrFlags::Pop as i32)
                        .is_err()
                    {
                        return Err(anyhow::anyhow!("No array variable with that name found"));
                    }
                }
                Ok(())
            }),
        }));
        list.push(ConsoleEntry::Cmd(ConsoleEntryCmd {
            name: "rem".into(),
            usage: "rem <var>[key]".into(),
            cmd: Rc::new(|config_engine, config_game, path| {
                if config_engine
                    .try_set_from_str(
                        path.clone(),
                        None,
                        None,
                        None,
                        ConfigFromStrFlags::Rem as i32,
                    )
                    .is_err()
                {
                    if config_game
                        .try_set_from_str(path, None, None, None, ConfigFromStrFlags::Rem as i32)
                        .is_err()
                    {
                        return Err(anyhow::anyhow!("No record variable with that key found"));
                    }
                }
                Ok(())
            }),
        }));
        list.push(ConsoleEntry::Cmd(ConsoleEntryCmd {
            name: "quit".into(),
            usage: "quit the client".into(),
            cmd: Rc::new(move |_, _, _| {
                console_events.borrow_mut().push(ConsoleEvent::Quit);
                Ok(())
            }),
        }));
    }

    pub fn new(
        config_game: &mut ConfigGame,
        config_engine: &mut ConfigEngine,
        native: &mut dyn NativeImpl,
    ) -> Self {
        let mut ui = UI::new(
            UIWinitWrapper {
                state: State::new(native.borrow_window()),
            },
            None,
        );
        ui.set_main_panel_color(&Color32::TRANSPARENT);

        let console_events: Rc<RefCell<Vec<ConsoleEvent>>> = Default::default();
        let mut entries: Vec<ConsoleEntry> = Vec::new();
        let val = ConfigEngine::conf_value();
        Self::parse_conf_values_as_str_list("".to_string(), &mut entries, val, false);
        let val = ConfigGame::conf_value();
        Self::parse_conf_values_as_str_list("".to_string(), &mut entries, val, false);
        Self::register_commands(console_events.clone(), &mut entries);

        let cmd = native.start_arguments().join(" ");
        if !cmd.is_empty() {
            client_ui::console::utils::run_command(
                &cmd,
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
            console_ui: ConsoleUI {},
            console_events,
        }
    }

    #[must_use]
    pub fn render(&mut self, pipe: &mut ConsoleRenderPipe) -> Vec<ConsoleEvent> {
        let mut ui_feedback = ClientStatsUIFeedbackDummy {};
        let mut ui_pipe = UIPipe::new(
            &mut ui_feedback,
            pipe.sys.time_get_nanoseconds(),
            pipe.config_engine,
            UserData {
                entries: &self.entries,
                config_game: pipe.config_game,
                msgs: pipe.msgs,
                msg: &mut self.text,
                select_index: &mut self.selected_index,
            },
        );
        let input_generator = UIWinitWrapperPipe {
            window: pipe.window,
        };
        let mut dummy_native_ui_pipe = UINativePipe {
            raw_inp_generator: &UIWinitWrapperDummyPipe {},
        };
        let mut native_ui_pipe = UINativePipe {
            raw_inp_generator: &input_generator,
        };

        generic_ui_renderer::render(
            pipe.graphics,
            &mut self.ui,
            &mut self.console_ui,
            &mut (),
            &mut (),
            &mut ui_pipe,
            &mut dummy_native_ui_pipe,
            &mut native_ui_pipe,
        );

        self.get_events()
    }

    #[must_use]
    pub fn get_events(&self) -> Vec<ConsoleEvent> {
        std::mem::take(&mut self.console_events.borrow_mut())
    }
}
