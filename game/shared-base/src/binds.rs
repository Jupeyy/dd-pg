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
    ShowScoreboard,
}

#[derive(Clone)]
pub enum BindActionsHotkey {
    Screenshot,
    Console,
}
