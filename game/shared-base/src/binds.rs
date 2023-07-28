#[derive(Clone)]
pub enum BindActionsLocalPlayer {
    MoveLeft,
    MoveRight,
    Jump,
    Fire,
    Hook,
}

#[derive(Clone)]
pub enum BindActions {
    LocalPlayer(BindActionsLocalPlayer),
}
