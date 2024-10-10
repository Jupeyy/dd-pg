#![allow(clippy::too_many_arguments)]
#![allow(clippy::module_inception)]
#![allow(clippy::multiple_bound_locations)]

pub mod collision;
pub mod config;
pub mod ctf_controller;
pub mod entities;
pub mod events;
pub mod game_objects;
pub mod match_manager;
pub mod match_state;
pub mod simulation_pipe;
pub mod snapshot;
pub mod spawns;
/// basic sql support
pub mod sql;
pub mod stage;
pub mod state;
pub mod types;
pub mod weapons;
pub mod world;

#[cfg(test)]
mod test {
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };

    use base_io::{io::create_runtime, io_batcher::IoBatcher};
    use game_database::dummy::DummyDb;
    use game_interface::{
        interface::{GameStateCreate, GameStateCreateOptions, GameStateInterface},
        types::{
            character_info::NetworkCharacterInfo,
            game::GameEntityId,
            input::{CharacterInput, CharacterInputCursor, CharacterInputInfo},
            network_stats::PlayerNetworkStats,
            player_info::{PlayerClientInfo, PlayerUniqueId},
        },
    };
    use hashlink::LinkedHashMap;
    use math::math::{vector::dvec2, Rng};
    use pool::pool::Pool;

    use crate::state::state::GameState;

    #[test]
    fn benchmark() {
        let file = include_bytes!("../../../data/map/maps/ctf1.twmap");

        let rt = create_runtime();
        let io_batcher = IoBatcher::new(rt);
        let (mut game, _) = GameState::new(
            file.to_vec(),
            "ctf1".to_string(),
            GameStateCreateOptions::default(),
            io_batcher,
            Arc::new(DummyDb),
        );

        let mut rng = Rng::new(0);

        const NUM_PLAYERS: usize = 64;

        let mut inps = vec![CharacterInput::default(); NUM_PLAYERS];

        let game_inps: Pool<LinkedHashMap<GameEntityId, CharacterInputInfo>> =
            Pool::with_capacity(1);

        let mut next_inp = |inps: &mut LinkedHashMap<GameEntityId, CharacterInputInfo>,
                            inp: &mut CharacterInput,
                            id: &GameEntityId,
                            force_dir: bool| {
            let mut new_inp = *inp;
            new_inp.state.fire.set(rng.random_int_in(0..=1) != 0);
            new_inp.state.hook.set(rng.random_int_in(0..=1) != 0);
            new_inp.state.jump.set(rng.random_int_in(0..=1) != 0);
            new_inp.state.dir.set(rng.random_int_in(0..=2) as i32 - 1);
            if force_dir && *new_inp.state.dir == 0 {
                new_inp.state.dir.set(-1);
            }
            new_inp
                .cursor
                .set(CharacterInputCursor::from_vec2(&dvec2::new(
                    rng.random_float() as f64,
                    rng.random_float() as f64,
                )));

            let diff = new_inp.consumable.diff(&inp.consumable);
            *inp = new_inp;
            inps.insert(*id, CharacterInputInfo { inp: new_inp, diff });
        };

        let ids: Vec<_> = (0..NUM_PLAYERS)
            .map(|index| {
                let id = game.player_join(&PlayerClientInfo {
                    info: NetworkCharacterInfo::explicit_default(),
                    is_dummy: false,
                    player_index: 0,
                    unique_identifier: PlayerUniqueId::Account(0),
                    initial_network_stats: PlayerNetworkStats::default(),
                });

                for _ in 0..2 {
                    let mut game_inps = game_inps.new();
                    next_inp(&mut game_inps, &mut inps[index], &id, true);
                    game.set_player_inputs(game_inps);
                    game.tick(Default::default());
                }

                id
            })
            .collect();

        let mut ticks: u64 = 0;
        let now = Instant::now();
        loop {
            let mut game_inps = game_inps.new();
            for (inp, id) in inps.iter_mut().zip(ids.iter()) {
                next_inp(&mut game_inps, inp, id, false);
            }
            game.set_player_inputs(game_inps);

            game.tick(Default::default());

            ticks += 1;
            if Instant::now().duration_since(now) >= Duration::from_secs(5) {
                break;
            }
        }

        println!("{} t/s", ticks / 5);
    }
}
