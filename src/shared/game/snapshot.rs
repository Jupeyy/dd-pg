

use super::{
    entities::character::{Character, CharacterCore},
    stage::GameStage,
    state::GameState,
    TGameElementID, INVALID_GAME_ELEMENT_ID,
};
use bincode::{Decode, Encode};

pub struct SnapshotClientInfo {
    pub client_player_id: TGameElementID,
    pub snap_everything: bool,
    pub snap_other_stages: bool,
    pub time_since_connect_nanos: u64,
}

#[derive(Encode, Decode, Default)]
pub struct SnapshotCharacter {
    pub core: CharacterCore,

    pub game_el_id: TGameElementID,
}

#[derive(Encode, Decode, Default)]
pub struct SnapshotWorld {
    pub characters: Vec<SnapshotCharacter>,
}

#[derive(Encode, Decode, Default)]
pub struct SnapshotStage {
    pub world: SnapshotWorld,

    pub game_el_id: TGameElementID,
}

#[derive(Encode, Decode, Default)]
pub struct Snapshot {
    pub stages: Vec<SnapshotStage>,
    pub game_tick: u64,

    // the monotonic_tick is monotonic increasing
    // it's not related to the game tick and reflects
    // the ticks passed since the server started
    pub monotonic_tick: u64,

    pub recv_player_id: TGameElementID,
    pub time_since_connect_nanos: u64,
}

pub struct SnapshotManager {
    pub helper_state: GameState,
}

impl SnapshotManager {
    pub fn new() -> Self {
        Self {
            helper_state: GameState::new(),
        }
    }

    pub fn build_for(&self, game: &GameState, client: &SnapshotClientInfo) -> Snapshot {
        let mut res = Snapshot::default();
        res.time_since_connect_nanos = client.time_since_connect_nanos;
        res.monotonic_tick = game.cur_monotonic_tick;
        res.recv_player_id = client.client_player_id;
        game.get_stages().iter().for_each(|stage| {
            res.stages.push(SnapshotStage {
                world: SnapshotWorld {
                    characters: stage
                        .get_world()
                        .get_characters()
                        .iter()
                        .map(|char| -> SnapshotCharacter {
                            SnapshotCharacter {
                                core: char.cores[0],
                                game_el_id: char.base.game_element_id,
                            }
                        })
                        .collect(),
                },
                game_el_id: stage.game_element_id,
            });
        });
        res
    }

    /**
     * Writes a snapshot into a game state
     * It uses a mutable reference to reuse vector capacity, heap objects etc.
     */
    pub fn convert_to_game_state(&mut self, snapshot: &Snapshot, write_game_state: &mut GameState) {
        // clear stages, we want to find stages that are in the snapshot aswell as in the game state
        // so we can reuse them
        self.helper_state.get_stages_mut().clear();
        for stage in write_game_state.get_stages_mut().drain(..) {
            let it = snapshot
                .stages
                .iter()
                .find(|stage_snap| stage_snap.game_el_id == stage.game_element_id);
            match it {
                Some(_) => self.helper_state.get_stages_mut().push(stage),
                None => {}
            }
        }

        // now add the stages in order of the snapshot
        snapshot.stages.iter().for_each(|stage_snap| {
            // if helper state contains the snapshot stage use that
            let it = self
                .helper_state
                .get_stages_mut()
                .iter_mut()
                .find(|stage| stage.game_element_id == stage_snap.game_el_id);
            match it {
                Some(game_stage) => {
                    let mut stage = GameStage::new(0, INVALID_GAME_ELEMENT_ID);
                    std::mem::swap(&mut stage, game_stage);
                    write_game_state.get_stages_mut().push(stage);
                }
                None => {
                    // create new stage
                    let stage = GameStage::new(0, stage_snap.game_el_id);
                    write_game_state.get_stages_mut().push(stage);
                }
            }

            let stage = write_game_state.get_stages_mut().last_mut().unwrap();
            stage.get_world_mut().get_characters_mut().clear();
            // now go through the children of the stage
            stage_snap.world.characters.iter().for_each(|char| {
                let mut character = Character::new(&char.game_el_id, &char.core.player_id);
                character.cores[0] = char.core;
                stage.get_world_mut().get_characters_mut().push(character);
            });
        });
    }
}
