use game_interface::types::weapons::WeaponType;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
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
    Kill,
    ToggleDummyCopyMoves,
    ToggleDummyHammerFly,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum BindActionsHotkey {
    Screenshot,
    Console,
    ConsoleClose,
}
