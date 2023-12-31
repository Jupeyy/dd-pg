use std::collections::VecDeque;

use client_types::chat::ServerMsg;

pub trait ChatInterface {
    fn on_message(&mut self, _msg: String) {
        panic!("not implemented")
    }
}

pub struct UserData<'a> {
    pub entries: &'a VecDeque<ServerMsg>,
    pub msg: &'a mut String,
    pub is_input_active: &'a mut bool,
    pub is_chat_show_all: bool,
    pub chat: &'a mut dyn ChatInterface,
}
