use std::{sync::Arc, time::Duration};

use anyhow::anyhow;
use client_map::client_map::GameMap;
use demo::DemoEvent;
use game_config::config::ConfigGame;
use game_interface::{
    interface::GameStateInterface,
    types::{
        game::{GameTickType, NonZeroGameTickType},
        input::CharacterInputInfo,
        snapshot::{FromSnapshotBuildMode, SnapshotClientInfo},
    },
};
use pool::rc::PoolRc;
use server::server::Server;
use shared::game::state_wasm_manager::GameStateWasmManager;
use shared_base::{
    game_types::time_until_tick,
    network::{messages::MsgClSnapshotAck, server_info::ServerInfo},
};
use shared_network::messages::{MsgSvLoadVotes, ServerToClientMessage};

use crate::{
    client::component::GameMsgPipeline,
    game::data::{GameData, SnapshotStorageItem},
    localplayer::{ClientPlayer, ServerInputForDiff},
};

/// This component makes sure the client sends the
/// network events based on the current state on the server
/// E.g. if the map is about to connect
/// but requires to load the map,
/// it needs to send the "Ready" event as soon as the client
/// is ready.
pub struct NetworkLogic {}

impl Default for NetworkLogic {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkLogic {
    pub fn new() -> Self {
        Self {}
    }

    fn ack_input(player: &mut ClientPlayer, input_id: u64) {
        if let Some(inp) = player.server_input_storage.remove(&input_id) {
            player.server_input = Some(ServerInputForDiff { id: input_id, inp });
        }
        while player
            .server_input_storage
            .first_entry()
            .is_some_and(|entry| *entry.key() < input_id)
        {
            player.server_input_storage.pop_first();
        }
    }

    pub fn on_msg(
        &mut self,
        timestamp: &Duration,
        msg: ServerToClientMessage<'static>,
        pipe: &mut GameMsgPipeline,
    ) {
        match msg {
            ServerToClientMessage::ServerInfo { .. } => {
                // TODO: update some stuff or just ignore?
            }
            ServerToClientMessage::Snapshot {
                overhead_time,
                mut snapshot,
                game_monotonic_tick_diff,
                snap_id_diffed,
                diff_id,
                as_diff,
                input_ack,
            } => {
                // first handle input acks, so no early returns can prevent that.
                for player in pipe.game_data.local_players.values_mut() {
                    for input in input_ack.iter() {
                        Self::ack_input(player, input.id);
                    }
                }

                // add the estimated ping to our prediction timer
                for input in input_ack.iter() {
                    if let Some(sent_at) = pipe.game_data.sent_input_ids.remove(&input.id) {
                        pipe.game_data.prediction_timer.add_ping(
                            timestamp
                                .saturating_sub(sent_at)
                                .saturating_sub(input.logic_overhead),
                            *timestamp,
                        );
                    }
                }

                let snapshot_and_id = if let Some(diff_id) = diff_id {
                    pipe.game_data.snap_storage.get(&diff_id)
                        .map(|old| {
                            let mut patch = pipe.game_data.player_snap_pool.new();
                            patch.resize(snapshot.len(), Default::default());
                            patch.clone_from_slice(snapshot.as_ref());
                            snapshot.to_mut().clear();
                            let patch_res = bin_patch::patch(&old.snapshot, &patch,  snapshot.to_mut());
                            patch_res.map(|_| (snapshot, game_monotonic_tick_diff + old.monotonic_tick)).map_err(|err| anyhow!(err))
                        }).unwrap_or_else(|| Err(anyhow!("patching snapshot difference failed, because the previous snapshot was missing.")))
                        .map(|(snap, game_monotonic_tick)| (snap, snap_id_diffed + diff_id, game_monotonic_tick))
                } else {
                    Ok((snapshot, snap_id_diffed, game_monotonic_tick_diff))
                };
                let (snapshot, snap_id, game_monotonic_tick) = match snapshot_and_id {
                    Ok((snapshot, snap_id, game_monotonic_tick)) => {
                        (snapshot, snap_id, game_monotonic_tick)
                    }
                    Err(err) => {
                        log::debug!(target: "network_logic", "had to drop a snapshot from the server with diff_id {:?}: {err}", diff_id);
                        return;
                    }
                };

                if let Some(demo_recorder) = pipe.demo_recorder {
                    demo_recorder.add_snapshot(game_monotonic_tick, snapshot.as_ref().to_vec());
                }

                let GameMap { game, .. } = pipe.map;
                let ticks_per_second = game.game_tick_speed();
                let tick_time = time_until_tick(ticks_per_second);
                let monotonic_tick = game_monotonic_tick;

                let mut prev_tick = game.predicted_game_monotonic_tick;
                if !pipe
                    .game_data
                    .handled_snap_id
                    .is_some_and(|id| id >= snap_id)
                {
                    // Reset cur state snap for future tick
                    pipe.game_data.cur_state_snap = None;

                    pipe.game_data.handled_snap_id = Some(snap_id);
                    if as_diff {
                        // this should be higher than the number of snapshots saved on the server
                        // (since reordering of packets etc.)
                        while pipe.game_data.snap_storage.len() >= 50 {
                            pipe.game_data.snap_storage.pop_first();
                        }
                        pipe.game_data.snap_storage.insert(
                            snap_id,
                            SnapshotStorageItem {
                                snapshot: std::mem::take(&mut *snapshot.clone().to_mut()),
                                monotonic_tick: game_monotonic_tick,
                            },
                        );
                    }
                    pipe.game_data.snap_acks.push(MsgClSnapshotAck { snap_id });

                    let predicted_game_monotonic_tick = monotonic_tick.max(prev_tick);

                    // if the incoming snapshot is older than the prediction tick, then we can use it directly
                    let snapshot =
                        if monotonic_tick < prev_tick || pipe.game_data.last_snap.is_none() {
                            pipe.game_data.last_snap = Some((snapshot, monotonic_tick));
                            None
                        } else if monotonic_tick == prev_tick {
                            Some(snapshot)
                        } else {
                            None
                        };

                    fn advance_game_state(
                        prev_tick: &mut GameTickType,
                        monotonic_tick: GameTickType,
                        game_data: &mut GameData,
                        game: &mut GameStateWasmManager,
                        ticks_per_second: NonZeroGameTickType,
                        config_game: &ConfigGame,
                        shared_info: &Arc<ServerInfo>,
                        timestamp: &Duration,
                        tick_time: Duration,
                        overhead_time: Option<Duration>,
                        after_tick: impl Fn(&mut GameStateWasmManager, &mut GameData, GameTickType),
                    ) {
                        match (*prev_tick).cmp(&monotonic_tick) {
                            std::cmp::Ordering::Greater => {
                                let max_tick = *prev_tick;
                                // the clamp ensures that the game at most predicts 3 seconds back, to prevent major fps drops
                                let min_tick = monotonic_tick.clamp(
                                    prev_tick.saturating_sub(game.game_tick_speed().get() * 3),
                                    *prev_tick,
                                );
                                (min_tick..max_tick).for_each(|new_tick| {
                                    // apply the player input if the tick had any
                                    let prev_tick_of_inp = new_tick;
                                    let tick_of_inp = new_tick + 1;
                                    if let (Some(inp), prev_inp) = (
                                        game_data.input_per_tick.get(&tick_of_inp).or_else(|| {
                                            game_data.input_per_tick.iter().rev().find_map(
                                                |(&tick, inp)| (tick <= tick_of_inp).then_some(inp),
                                            )
                                        }),
                                        game_data.input_per_tick.get(&prev_tick_of_inp),
                                    ) {
                                        let mut inps = game_data.player_inputs_state_pool.new();
                                        inp.iter().for_each(|(player_id, player_inp)| {
                                            let mut prev_player_inp = prev_inp
                                                .or(Some(inp))
                                                .and_then(|inps| inps.get(player_id).cloned())
                                                .unwrap_or_default();

                                            if let Some(diff) = prev_player_inp.try_overwrite(
                                                &player_inp.inp,
                                                player_inp.version(),
                                                true,
                                            ) {
                                                inps.insert(
                                                    *player_id,
                                                    CharacterInputInfo {
                                                        inp: player_inp.inp,
                                                        diff,
                                                    },
                                                );
                                            }
                                        });
                                        game.set_player_inputs(inps);
                                    }
                                    game.tick(Default::default());
                                    after_tick(game, game_data, new_tick + 1);

                                    Server::dbg_game(
                                        &config_game.dbg,
                                        &game_data.last_game_tick,
                                        game,
                                        game_data
                                            .input_per_tick
                                            .get(&tick_of_inp)
                                            .map(|inps| inps.values().map(|inp| &inp.inp)),
                                        new_tick + 1,
                                        ticks_per_second.get(),
                                        shared_info,
                                        "client-pred",
                                    );
                                });
                            }
                            std::cmp::Ordering::Less => {
                                if let Some(overhead_time) = overhead_time {
                                    game_data.last_game_tick = timestamp
                                        .saturating_sub(
                                            game_data.prediction_timer.pred_max_smooth(tick_time),
                                        )
                                        .saturating_sub(overhead_time);
                                    *prev_tick = monotonic_tick;
                                }
                            }
                            std::cmp::Ordering::Equal => {
                                // ignore
                            }
                        }
                    }

                    // advance the previous state to to just before the prediction tick
                    if let Some((prev_snapshot, prev_state_tick)) = &pipe.game_data.last_snap {
                        let local_players =
                            game.build_from_snapshot(prev_snapshot, Default::default());
                        // set local players
                        GameData::handle_local_players_from_snapshot(
                            &mut pipe.game_data.local_players,
                            pipe.config_game,
                            pipe.console_entries,
                            &local_players,
                        );
                        let prev_state_tick = *prev_state_tick;
                        advance_game_state(
                            &mut predicted_game_monotonic_tick.saturating_sub(1),
                            prev_state_tick,
                            pipe.game_data,
                            game,
                            ticks_per_second,
                            pipe.config_game,
                            pipe.shared_info,
                            timestamp,
                            tick_time,
                            None,
                            |game, game_data, tick| {
                                // if anti ping is off, make sure to revert all changes that are not desired
                                if !pipe.config_game.cl.anti_ping {
                                    if let Some((prev_snapshot, _)) = &game_data.last_snap {
                                        let prev_now_tick_diff =
                                            monotonic_tick.saturating_sub(prev_state_tick);
                                        if tick
                                            == predicted_game_monotonic_tick
                                                .saturating_sub(1)
                                                .saturating_sub(prev_now_tick_diff)
                                        {
                                            let mut ids = game_data.player_ids_pool.new();
                                            ids.extend(local_players.keys());
                                            let _ = game.build_from_snapshot(
                                                prev_snapshot,
                                                FromSnapshotBuildMode::Unpredicted {
                                                    keep_characters: ids,
                                                },
                                            );
                                        }
                                    }
                                }
                            },
                        );

                        let prev_state_snap = game.snapshot_for(SnapshotClientInfo::Everything);
                        game.build_from_snapshot_for_prev(&prev_state_snap);
                    }

                    let can_unpredict = snapshot.is_some();
                    let advance_from_monotonic_tick = if let Some(snapshot) = snapshot {
                        let local_players = game.build_from_snapshot(&snapshot, Default::default());
                        // set local players
                        GameData::handle_local_players_from_snapshot(
                            &mut pipe.game_data.local_players,
                            pipe.config_game,
                            pipe.console_entries,
                            &local_players,
                        );

                        pipe.game_data.last_snap = Some((snapshot, monotonic_tick));
                        monotonic_tick
                    } else {
                        predicted_game_monotonic_tick.saturating_sub(1)
                    };

                    game.predicted_game_monotonic_tick = predicted_game_monotonic_tick;

                    advance_game_state(
                        &mut prev_tick,
                        advance_from_monotonic_tick,
                        pipe.game_data,
                        game,
                        ticks_per_second,
                        pipe.config_game,
                        pipe.shared_info,
                        timestamp,
                        tick_time,
                        Some(overhead_time),
                        |game, game_data, tick| {
                            // if anti ping is off, make sure to revert all changes that are not desired
                            if !pipe.config_game.cl.anti_ping
                                && can_unpredict
                                && tick == predicted_game_monotonic_tick
                            {
                                if let Some((snapshot, _)) = &game_data.last_snap {
                                    let mut ids = game_data.player_ids_pool.new();
                                    ids.extend(game_data.local_players.keys());
                                    let _ = game.build_from_snapshot(
                                        snapshot,
                                        FromSnapshotBuildMode::Unpredicted {
                                            keep_characters: ids,
                                        },
                                    );
                                }
                            }
                        },
                    );

                    // drop queued input that was before or at the server monotonic tick
                    while pipe
                        .game_data
                        .input_per_tick
                        .front()
                        .is_some_and(|(&tick, _)| tick < monotonic_tick)
                    {
                        pipe.game_data.input_per_tick.pop_front();
                    }
                }
                let prediction_timer = &mut pipe.game_data.prediction_timer;
                let predict_max = prediction_timer.pred_max_smooth(tick_time);
                let ticks_in_pred = (predict_max.as_nanos() / tick_time.as_nanos()) as u64;
                let time_in_pred =
                    Duration::from_nanos((predict_max.as_nanos() % tick_time.as_nanos()) as u64);

                // we remove the overhead of the server here,
                // the reason is simple: if the server required 10ms for 63 players snapshots
                // the 64th player's client would "think" it runs 10ms behind and speeds up
                // computation, but the inputs are handled much earlier on the server then.
                let timestamp = timestamp.saturating_sub(overhead_time);
                let time_diff =
                    timestamp.as_secs_f64() - pipe.game_data.last_game_tick.as_secs_f64();
                let pred_tick = prev_tick;

                let tick_diff =
                    (pred_tick as i128 - monotonic_tick as i128) as f64 - ticks_in_pred as f64;
                let time_diff = time_diff - time_in_pred.as_secs_f64();

                let time_diff = tick_diff * tick_time.as_secs_f64() + time_diff;

                prediction_timer.add_snap(time_diff, timestamp);
            }
            ServerToClientMessage::Events {
                events,
                game_monotonic_tick,
            } => {
                if let Some(demo_recorder) = pipe.demo_recorder {
                    demo_recorder.add_event(game_monotonic_tick, DemoEvent::Game(events.clone()));
                }

                let event_id = events.event_id;
                pipe.events.insert(game_monotonic_tick, (events, false));
                pipe.map.game.sync_event_id(event_id);
            }
            ServerToClientMessage::Load(_) => {
                panic!("this should be handled by earlier logic.");
            }
            ServerToClientMessage::QueueInfo(_) => {
                // ignore
            }
            ServerToClientMessage::Chat(chat_msg) => {
                if let Some(demo_recorder) = pipe.demo_recorder {
                    demo_recorder.add_event(
                        pipe.map.game.predicted_game_monotonic_tick,
                        DemoEvent::Chat(chat_msg.msg.clone()),
                    );
                }

                pipe.game_data.chat_msgs.push_back(chat_msg.msg);
            }
            ServerToClientMessage::Vote(vote_state) => {
                let voted = pipe
                    .game_data
                    .vote
                    .as_ref()
                    .and_then(|(_, voted, _)| *voted);
                pipe.game_data.vote =
                    vote_state.map(|v| (PoolRc::from_item_without_pool(v), voted, *timestamp));
            }
            ServerToClientMessage::LoadVote(votes) => match votes {
                MsgSvLoadVotes::Map { votes } => {
                    pipe.game_data.map_votes = votes;
                }
                MsgSvLoadVotes::Misc { votes } => {
                    pipe.game_data.misc_votes = votes;
                }
            },
            ServerToClientMessage::RconCommands(cmds) => {
                pipe.remote_console.fill_entries(cmds.cmds);
            }
            ServerToClientMessage::AccountRenameRes(new_name) => match new_name {
                Ok(new_name) => {
                    pipe.account_info.fill_last_action_response(Some(None));
                    if let Some((mut account_info, creation_date)) =
                        pipe.account_info.account_info().clone()
                    {
                        account_info.name = new_name;
                        pipe.account_info
                            .fill_account_info(Some((account_info, creation_date)));
                    }
                }
                Err(err) => {
                    pipe.account_info.fill_last_action_response(Some(Some(err)));
                }
            },
            ServerToClientMessage::AccountDetails(details) => match details {
                Ok(details) => {
                    pipe.account_info.fill_last_action_response(None);
                    let creation_date = details
                        .creation_date
                        .to_chrono()
                        .map(|d| chrono::DateTime::<chrono::Local>::from(d).to_string())
                        .unwrap_or_default();
                    pipe.account_info
                        .fill_account_info(Some((details, creation_date)));
                }
                Err(err) => {
                    pipe.account_info.fill_last_action_response(Some(Some(err)));
                }
            },
            ServerToClientMessage::SpatialChat { entities } => {
                pipe.spatial_chat.on_input(
                    pipe.spatial_world
                        .as_deref_mut()
                        .map(|world| (world, pipe.map.game.collect_characters_info())),
                    entities,
                    pipe.config_game,
                );
            }
        }
    }
}
