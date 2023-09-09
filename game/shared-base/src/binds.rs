#[derive(Clone)]
pub enum BindActionsLocalPlayer {
    MoveLeft,
    MoveRight,
    Jump,
    Fire,
    Hook,
    OpenMenu,
}

#[derive(Clone)]
pub enum BindActionsHotkey {
    Screenshot,
}

#[derive(Clone)]
pub enum BindActions {
    LocalPlayer(BindActionsLocalPlayer),
    Hotkeys(BindActionsHotkey),
}
