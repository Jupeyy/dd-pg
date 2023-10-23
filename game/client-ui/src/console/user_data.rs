use client_types::console::ConsoleEntry;

pub struct UserData<'a> {
    pub entries: &'a Vec<ConsoleEntry>,
    pub msgs: &'a mut String,
    pub msg: &'a mut String,
}
