use std::{collections::HashMap, ops::Range, rc::Rc};

use command_parser::parser::{CommandArg, Syn};
use config::config::ConfigEngine;
use game_config::config::ConfigGame;

#[derive(Debug)]
pub struct ConsoleEntryVariable {
    pub full_name: String,
    pub usage: String,

    /// for parsing
    pub args: Vec<CommandArg>,
}

pub type ConsoleCmdCb =
    Rc<dyn Fn(&mut ConfigEngine, &mut ConfigGame, &[(Syn, Range<usize>)]) -> anyhow::Result<()>>;

pub struct ConsoleEntryCmd {
    pub name: String,
    pub usage: String,
    pub cmd: ConsoleCmdCb,

    /// for parsing
    pub args: Vec<CommandArg>,
}

pub enum ConsoleEntry {
    Var(ConsoleEntryVariable),
    Cmd(ConsoleEntryCmd),
}

pub fn entries_to_parser(entries: &[ConsoleEntry]) -> HashMap<String, Vec<CommandArg>> {
    entries
        .iter()
        .map(|entry| match entry {
            ConsoleEntry::Var(entry) => (entry.full_name.clone(), entry.args.clone()),
            ConsoleEntry::Cmd(entry) => (entry.name.clone(), entry.args.clone()),
        })
        .collect()
}
