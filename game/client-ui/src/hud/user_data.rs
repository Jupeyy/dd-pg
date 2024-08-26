use game_interface::types::{
    game::{GameTickType, NonZeroGameTickType},
    render::game::GameRenderInfo,
};

pub struct UserData<'a> {
    pub race_timer_counter: &'a GameTickType,
    pub ticks_per_second: &'a NonZeroGameTickType,
    pub game: Option<&'a GameRenderInfo>,
}
