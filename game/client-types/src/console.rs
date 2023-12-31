use std::rc::Rc;

use config::config::ConfigEngine;
use game_config::config::ConfigGame;

pub struct ConsoleEntryVariable {
    pub full_name: String,
    pub usage: String,
}

pub struct ConsoleEntryCmd {
    pub name: String,
    pub usage: String,
    pub cmd: Rc<dyn Fn(&mut ConfigEngine, &mut ConfigGame, String) -> anyhow::Result<()>>,
}

pub enum ConsoleEntry {
    Var(ConsoleEntryVariable),
    Cmd(ConsoleEntryCmd),
}
