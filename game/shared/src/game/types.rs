use math::math::vector::{dvec2, vec2};
use shared_base::game_types::{TGameElementID, INVALID_GAME_ELEMENT_ID};
use shared_game::state::state::GameStateInterface;

use super::state_wasm_manager::GameStateWasmManager;

pub struct PlayerAndCharacterProxy<'a> {
    pub player_id: TGameElementID,
    pub wasm_manager: &'a GameStateWasmManager,
}

impl<'a> PlayerAndCharacterProxy<'a> {
    pub fn lerp_core_pos(&self, ratio: f64) -> vec2 {
        self.wasm_manager.lerp_core_pos(&self.player_id, ratio)
    }
    pub fn lerp_core_vel(&self, ratio: f64) -> vec2 {
        self.wasm_manager.lerp_core_vel(&self.player_id, ratio)
    }
    pub fn lerp_core_hook_pos(&self, ratio: f64) -> vec2 {
        self.wasm_manager.lerp_core_hook_pos(&self.player_id, ratio)
    }
    pub fn cursor_vec2(&self) -> dvec2 {
        self.wasm_manager.cursor_vec2(&self.player_id)
    }
    pub fn input_dir(&self) -> i32 {
        self.wasm_manager.input_dir(&self.player_id)
    }
}

pub struct PlayerWithCharIter<'a> {
    pub player_id: TGameElementID,
    pub wasm_manager: &'a GameStateWasmManager,
}

impl<'a> Iterator for PlayerWithCharIter<'a> {
    type Item = PlayerAndCharacterProxy<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.player_id != INVALID_GAME_ELEMENT_ID {
            let player_id = self.player_id.clone();
            let next_id = self.wasm_manager.player_id_after_id(&self.player_id);
            self.player_id = next_id.unwrap_or(INVALID_GAME_ELEMENT_ID).clone();
            Some(PlayerAndCharacterProxy {
                player_id,
                wasm_manager: self.wasm_manager,
            })
        } else {
            None
        }
    }
}
