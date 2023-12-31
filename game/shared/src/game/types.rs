use math::math::vector::{dvec2, vec2};
use shared_base::game_types::TGameElementID;
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
