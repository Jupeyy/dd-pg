use crate::network::messages::WeaponType;

#[derive(Clone)]
pub enum BindActionsLocalPlayer {
    MoveLeft,
    MoveRight,
    Jump,
    Fire,
    Hook,
    NextWeapon,
    PrevWeapon,
    Weapon(WeaponType),
    OpenMenu,
    ActivateChatInput,
}

#[derive(Clone)]
pub enum BindActionsHotkey {
    Screenshot,
    Console,
}

#[derive(Clone)]
pub enum BindActions {
    LocalPlayer(BindActionsLocalPlayer),
    Hotkeys(BindActionsHotkey),
}
