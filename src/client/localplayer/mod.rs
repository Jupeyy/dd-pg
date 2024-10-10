use std::{collections::BTreeMap, time::Duration};

use binds::binds::BindActions;
use client_ui::emote_wheel::user_data::EmoteWheelEvent;
use game_interface::types::{
    game::{GameEntityId, GameTickType},
    render::character::PlayerCameraMode,
};
use hashlink::LinkedHashMap;
use math::math::vector::dvec2;
use native::input::binds::Binds;
use pool::{
    datatypes::{PoolLinkedHashMap, PoolVec},
    pool::Pool,
};
use shared_base::{network::messages::PlayerInputChainable, player_input::PlayerInput};

pub type ClientPlayerInputPerTick =
    LinkedHashMap<GameTickType, PoolLinkedHashMap<GameEntityId, PlayerInput>>;

#[derive(Debug)]
pub struct ServerInputForDiff {
    pub id: u64,
    pub inp: PlayerInputChainable,
}

#[derive(Debug, Default)]
pub struct ClientPlayer {
    pub input: PlayerInput,
    pub sent_input: PlayerInput,
    pub sent_input_time: Option<Duration>,
    /// The game tick the input was sent in
    pub sent_inp_tick: GameTickType,

    pub binds: Binds<Vec<BindActions>>,

    pub chat_input_active: bool,
    pub chat_msg: String,

    /// show a longer chat history
    pub show_chat_all: bool,
    pub show_scoreboard: bool,

    pub emote_wheel_active: bool,
    pub last_emote_wheel_selection: Option<EmoteWheelEvent>,

    // dummy controls
    pub dummy_copy_moves: bool,
    pub dummy_hammer: bool,

    /// For updating the player info on the server.
    pub player_info_version: u64,

    /// last input the server knows about
    pub server_input: Option<ServerInputForDiff>,
    /// inputs the client still knows about,
    /// [`PlayerInputChainable`] here is always the last of a chain that is send.
    pub server_input_storage: BTreeMap<u64, PlayerInputChainable>,

    pub is_dummy: bool,

    pub zoom: f32,

    pub input_cam_mode: PlayerCameraMode,
    pub free_cam_pos: dvec2,
    pub cursor_pos: dvec2,
}

pub type LocalPlayers = LinkedHashMap<GameEntityId, ClientPlayer>;

impl ClientPlayer {
    pub fn get_and_update_latest_input(
        local_players: &mut LocalPlayers,
        cur_time: Duration,
        time_per_tick: Duration,
        ticks_to_send: GameTickType,
        tick_of_inp: GameTickType,
        player_inputs: &mut LinkedHashMap<GameEntityId, PoolVec<PlayerInputChainable>>,
        player_inputs_chainable_pool: &Pool<Vec<PlayerInputChainable>>,
        tick_inps: &ClientPlayerInputPerTick,
    ) {
        let mut copied_input = None;
        for (local_player_id, local_player) in local_players.iter_mut() {
            if local_player.dummy_copy_moves {
                copied_input = Some(local_player.input.inp);
            } else if let Some(copied_input) =
                &copied_input.and_then(|copied_input| local_player.is_dummy.then_some(copied_input))
            {
                local_player.input.try_overwrite(
                    copied_input,
                    local_player.input.version() + 1,
                    true,
                );
            }

            let should_send_rates = !local_player
                .sent_input_time
                .is_some_and(|time| cur_time - time < time_per_tick);
            let consumable_input_changed =
                local_player.sent_input.inp.consumable != local_player.input.inp.consumable;
            let send_by_input_change = (consumable_input_changed
                && (!local_player
                    .input
                    .inp
                    .consumable
                    .only_weapon_diff_changed(&local_player.sent_input.inp.consumable)
                    || should_send_rates))
                || local_player.sent_input.inp.state != local_player.input.inp.state
                || (local_player.sent_input.inp.cursor != local_player.input.inp.cursor
                    && should_send_rates);
            let should_send_old_input =
                tick_of_inp.saturating_sub(local_player.sent_inp_tick) < ticks_to_send;
            if send_by_input_change || (should_send_old_input && should_send_rates) {
                local_player.sent_input_time = Some(cur_time);

                if send_by_input_change {
                    local_player.sent_inp_tick = tick_of_inp;
                }

                let net_inp = &mut local_player.input;
                net_inp.inc_version();
                local_player.sent_input = *net_inp;

                let player_input_chains = player_inputs
                    .entry(*local_player_id)
                    .or_insert_with(|| player_inputs_chainable_pool.new());

                for tick in
                    tick_of_inp.saturating_sub(ticks_to_send.saturating_sub(1))..=tick_of_inp
                {
                    if tick != tick_of_inp {
                        // look for old inputs from previous ticks
                        if let Some(old_inp) = tick_inps
                            .get(&tick)
                            .and_then(|inps| inps.get(local_player_id))
                        {
                            player_input_chains.push(PlayerInputChainable {
                                for_monotonic_tick: tick,
                                inp: *old_inp,
                            });
                        }
                    } else {
                        player_input_chains.push(PlayerInputChainable {
                            for_monotonic_tick: tick_of_inp,
                            inp: *net_inp,
                        });
                    }
                }
            }
        }
    }
}
