use std::{collections::HashMap, ops::Range, rc::Rc};

use config::config::ConfigEngine;
use game_config::config::ConfigGame;

use self::parser::{CommandArg, Syn};

pub mod parser;
pub mod tokenizer;

pub struct ConsoleEntryVariable {
    pub full_name: String,
    pub usage: String,

    /// for parsing
    pub args: Vec<CommandArg>,
}

pub struct ConsoleEntryCmd {
    pub name: String,
    pub usage: String,
    pub cmd: Rc<
        dyn Fn(&mut ConfigEngine, &mut ConfigGame, &[(Syn, Range<usize>)]) -> anyhow::Result<()>,
    >,

    /// for parsing
    pub args: Vec<CommandArg>,
}

pub enum ConsoleEntry {
    Var(ConsoleEntryVariable),
    Cmd(ConsoleEntryCmd),
}

pub fn entries_to_parser(entries: &Vec<ConsoleEntry>) -> HashMap<String, Vec<CommandArg>> {
    entries
        .iter()
        .map(|entry| match entry {
            ConsoleEntry::Var(entry) => (entry.full_name.clone(), entry.args.clone()),
            ConsoleEntry::Cmd(entry) => (entry.name.clone(), entry.args.clone()),
        })
        .collect()
}
