use std::{collections::HashMap, rc::Rc};

use client_types::console::{ConsoleEntry, ConsoleEntryCmd};
use command_parser::parser::CommandArg;
use egui::Color32;
use hiarc::{hiarc_safer_rc_refcell, Hiarc};
use ui_base::ui::UiCreator;

use super::console::ConsoleRender;

#[derive(Debug, Hiarc)]
pub enum RemoteConsoleEvent {
    Exec(String),
}

#[hiarc_safer_rc_refcell]
#[derive(Debug, Default, Hiarc)]
pub struct RemoteConsoleEvents {
    events: Vec<RemoteConsoleEvent>,
}

#[hiarc_safer_rc_refcell]
impl RemoteConsoleEvents {
    pub fn push(&mut self, ev: RemoteConsoleEvent) {
        self.events.push(ev)
    }
}

#[hiarc_safer_rc_refcell]
impl super::console::ConsoleEvents<RemoteConsoleEvent> for RemoteConsoleEvents {
    #[hiarc_trait_is_immutable_self]
    fn take(&mut self) -> Vec<RemoteConsoleEvent> {
        std::mem::take(&mut self.events)
    }
}

pub type RemoteConsole = ConsoleRender<RemoteConsoleEvent, RemoteConsoleEvents>;

impl RemoteConsole {
    pub fn fill_entries(&mut self, cmds: HashMap<String, Vec<CommandArg>>) {
        self.entries.clear();
        for (name, args) in cmds {
            let cmds = self.user.clone();
            let name_clone = name.clone();
            self.entries.push(ConsoleEntry::Cmd(ConsoleEntryCmd {
                name,
                usage: "TODO:".into(),
                cmd: Rc::new(move |_config_engine, config_game, path| {
                    // TODO: add path
                    cmds.push(RemoteConsoleEvent::Exec(name_clone.clone()));
                    Ok(())
                }),
                args,
            }));
        }
    }
}

#[derive(Debug, Default)]
pub struct RemoteConsoleBuilder {}

impl RemoteConsoleBuilder {
    pub fn build(creator: &UiCreator) -> RemoteConsole {
        let console_events: RemoteConsoleEvents = Default::default();
        let entries: Vec<ConsoleEntry> = Vec::new();
        ConsoleRender::new(
            creator,
            entries,
            Box::new(console_events.clone()),
            Color32::from_rgba_unmultiplied(50, 0, 0, 150),
            console_events,
        )
    }
}
