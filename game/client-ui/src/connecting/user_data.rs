pub enum ConnectMode {
    Connecting,
    Queue { msg: String },
    Err { msg: String },
}

pub struct UserData<'a> {
    pub mode: &'a ConnectMode,
}
