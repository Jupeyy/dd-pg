use crate::network::messages::{MsgObjPlayerInput, WeaponType};

use super::{
    collision::Collision, entities::character_core::Core, TGameElementID, INVALID_GAME_ELEMENT_ID,
};

#[derive(Clone, Copy, Default)]
pub struct LocalPlayerInput {
    pub x: i32,
    pub y: i32,
    pub dir: i32,
    pub jump: bool,
    pub hook: bool,
}

impl LocalPlayerInput {
    pub fn from_net_obj(net_obj: &MsgObjPlayerInput) -> Self {
        Self {
            x: net_obj.cursor_x as i32,
            y: net_obj.cursor_y as i32,
            dir: net_obj.dir,
            jump: net_obj.jump,
            hook: net_obj.hook,
        }
    }

    pub fn to_net_obj(&self) -> MsgObjPlayerInput {
        MsgObjPlayerInput {
            cursor_x: self.x as i64,
            cursor_y: self.y as i64,
            dir: self.dir,
            jump: self.jump,
            hook: self.hook,

            fire: 0,
            flags: 0,
            weapon_req: WeaponType::TODO,
            weapon_diff: 0,
        }
    }
}

#[derive(Clone, Copy)]
pub struct ClientPlayer {
    pub input: LocalPlayerInput,
    pub player_id: TGameElementID,
}

pub struct LocalPlayers {
    pub players: [ClientPlayer; 4],
}

impl LocalPlayers {
    pub fn new() -> Self {
        Self {
            players: [
                ClientPlayer {
                    input: LocalPlayerInput {
                        x: 0,
                        y: 0,
                        dir: 0,
                        jump: false,
                        hook: false,
                    },
                    player_id: INVALID_GAME_ELEMENT_ID,
                },
                ClientPlayer {
                    input: LocalPlayerInput {
                        x: 0,
                        y: 0,
                        dir: 0,
                        jump: false,
                        hook: false,
                    },
                    player_id: INVALID_GAME_ELEMENT_ID,
                },
                ClientPlayer {
                    input: LocalPlayerInput {
                        x: 0,
                        y: 0,
                        dir: 0,
                        jump: false,
                        hook: false,
                    },
                    player_id: INVALID_GAME_ELEMENT_ID,
                },
                ClientPlayer {
                    input: LocalPlayerInput {
                        x: 0,
                        y: 0,
                        dir: 0,
                        jump: false,
                        hook: false,
                    },
                    player_id: INVALID_GAME_ELEMENT_ID,
                },
            ],
        }
    }
}

pub trait SimulationPlayerInput {
    fn get_input(&self, player_id: TGameElementID) -> Option<&LocalPlayerInput>;
}

impl SimulationPlayerInput for LocalPlayers {
    fn get_input(&self, player_id: TGameElementID) -> Option<&LocalPlayerInput> {
        let it = self.players.iter().find(|c| c.player_id == player_id);
        match it {
            Some(client) => Some(&client.input), // TODO
            None => None,
        }
    }
}

pub struct SimulationPipe<'a> {
    pub player_inputs: &'a dyn SimulationPlayerInput,
    pub collision: &'a Collision,
}

impl<'a> SimulationPipe<'a> {
    pub fn new(player_inputs: &'a dyn SimulationPlayerInput, collision: &'a Collision) -> Self {
        Self {
            player_inputs,
            collision: collision,
        }
    }
}

pub struct SimulationPipeStage<'a> {
    pub prev_core_index: usize,
    pub next_core_index: usize,

    pub player_input: &'a dyn SimulationPlayerInput,

    // should only be true inside a client's simulation pipe
    pub is_prediction: bool,

    pub collision: &'a Collision,
}

impl<'a> SimulationPipeStage<'a> {
    pub fn new(
        prev_core_index: usize,
        next_core_index: usize,
        player_input: &'a dyn SimulationPlayerInput,
        is_prediction: bool,
        collision: &'a Collision,
    ) -> Self {
        Self {
            next_core_index: next_core_index,
            prev_core_index: prev_core_index,
            player_input: player_input,
            is_prediction: is_prediction,
            collision: collision,
        }
    }
}

pub struct SimulationPipeEntities<'a> {
    pub prev_core_index: usize,
    pub next_core_index: usize,

    pub player_inputs: &'a dyn SimulationPlayerInput,
    pub other_chars_before: &'a mut [&'a mut Core],
    pub other_chars_after: &'a mut [&'a mut Core],

    pub collision: &'a Collision,
}

impl<'a> SimulationPipeEntities<'a> {
    pub fn new(
        prev_core_index: usize,
        next_core_index: usize,
        player_inputs: &'a dyn SimulationPlayerInput,
        other_chars_before: &'a mut [&'a mut Core],
        other_chars_after: &'a mut [&'a mut Core],
        collision: &'a Collision,
    ) -> Self {
        Self {
            next_core_index: next_core_index,
            prev_core_index: prev_core_index,
            player_inputs,
            other_chars_before: other_chars_before,
            other_chars_after: other_chars_after,
            collision: collision,
        }
    }
}
