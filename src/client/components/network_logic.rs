use std::time::Duration;

use anyhow::anyhow;
use client_demo::DemoRecorder;
use client_map::client_map::GameMap;
use game_interface::interface::GameStateInterface;
use server::server::Server;
use shared_base::game_types::time_until_tick;
use shared_network::messages::{ClientToServerMessage, GameMessage, ServerToClientMessage};

use crate::client::component::GameMsgPipeline;

/// This component makes sure the client sends the
/// network events based on the current state on the server
/// E.g. if the map is about to connect
/// but requires to load the map,
/// it needs to send the "Ready" event as soon as the client
/// is ready.
pub struct NetworkLogic {}

impl NetworkLogic {
    pub fn new() -> Self {
        Self {}
    }

    pub fn on_msg(
        &mut self,
        timestamp: &Duration,
        msg: ServerToClientMessage,
        pipe: &mut GameMsgPipeline,
    ) {
        match msg {
            ServerToClientMessage::ServerInfo { .. } => {
                // TODO: update some stuff or just ignore?
            }
            ServerToClientMessage::Snapshot {
                overhead_time,
                mut snapshot,
                game_monotonic_tick,
                snap_id,
                diff_id,
                as_diff,
            } => {
                let snapshot = if let Some(diff_id) = diff_id {
                    pipe.client_data
                        .prev_snapshots
                        .get(&diff_id)
                        .map(|old| {
                            let mut patch = pipe.client_data.player_snap_pool.new();
                            patch.clone_from(&snapshot);
                            snapshot.clear();
                            let patch_res = bin_patch::patch(old, &patch, &mut snapshot);
                            patch_res.map(|_| snapshot).map_err(|err| anyhow!(err))
                        }).unwrap_or_else(|| Err(anyhow!("patching snapshot difference failed, because the previous snapshot was missing.")))
                } else {
                    Ok(snapshot)
                };
                let Ok(mut snapshot) = snapshot else {
                    return;
                };

                if let Some(demo_recorder) = pipe.demo_recorder {
                    demo_recorder.add_snapshot(
                        bincode::serde::encode_to_vec(&snapshot, bincode::config::standard())
                            .unwrap(),
                    );
                }

                let GameMap { game, .. } = pipe.map;
                let ticks_per_second = game.game_tick_speed();
                let monotonic_tick = game_monotonic_tick;

                let prev_tick = game.predicted_game_monotonic_tick;
                if !pipe
                    .client_data
                    .handled_snap_id
                    .is_some_and(|id| id >= snap_id)
                {
                    let local_players = game.build_from_snapshot(&snapshot);
                    // set local players
                    pipe.client_data.handle_local_players_from_snapshot(
                        &pipe.config_game,
                        pipe.console_entries,
                        local_players,
                    );

                    pipe.client_data.handled_snap_id = Some(snap_id);
                    if let Some(diff_id) = diff_id {
                        while pipe
                            .client_data
                            .prev_snapshots
                            .first_entry()
                            .is_some_and(|entry| *entry.key() < diff_id)
                        {
                            pipe.client_data.prev_snapshots.pop_first();
                        }
                    }
                    let as_diff = as_diff && pipe.client_data.prev_snapshots.len() < 10;
                    if as_diff {
                        pipe.client_data
                            .prev_snapshots
                            .insert(snap_id, std::mem::take(&mut *snapshot));
                    }
                    pipe.network
                        .send_unreliable_to_server(&GameMessage::ClientToServer(
                            ClientToServerMessage::SnapshotAck { snap_id, as_diff },
                        ));

                    game.predicted_game_monotonic_tick = monotonic_tick.max(prev_tick);
                    // drop queued input that was before or at the server monotonic tick
                    while pipe
                        .client_data
                        .input_per_tick
                        .front()
                        .is_some_and(|(&tick, _)| tick < monotonic_tick)
                    {
                        pipe.client_data.input_per_tick.pop_front();
                    }
                    if prev_tick > monotonic_tick {
                        (monotonic_tick..prev_tick).for_each(|new_tick| {
                            // apply the player input if the tick had any
                            let prev_tick_of_inp = new_tick;
                            let tick_of_inp = new_tick + 1;
                            if let (Some(inp), prev_inp) = (
                                pipe.client_data
                                    .input_per_tick
                                    .get(&tick_of_inp)
                                    .or_else(|| {
                                        pipe.client_data.input_per_tick.iter().rev().find_map(
                                            |(&tick, inp)| (tick <= tick_of_inp).then_some(inp),
                                        )
                                    }),
                                pipe.client_data.input_per_tick.get(&prev_tick_of_inp),
                            ) {
                                inp.iter().for_each(|(player_id, player_inp)| {
                                    let mut prev_player_inp = prev_inp
                                        .or(Some(inp))
                                        .map(|inps| inps.get(player_id).cloned())
                                        .flatten()
                                        .unwrap_or_else(|| Default::default());

                                    if let Some(diff) = prev_player_inp.try_overwrite(
                                        &player_inp.inp,
                                        player_inp.version(),
                                        true,
                                    ) {
                                        game.set_player_input(player_id, &player_inp.inp, diff);
                                    }
                                });
                            }
                            game.tick();

                            Server::dbg_game(
                                &pipe.config_game.dbg,
                                &pipe.client_data.last_game_tick,
                                game,
                                pipe.client_data
                                    .input_per_tick
                                    .get(&tick_of_inp)
                                    .map(|inps| inps.values().map(|inp| &inp.inp)),
                                new_tick + 1,
                                ticks_per_second,
                                pipe.shared_info,
                                "client-pred",
                            );
                        });
                    }
                }
                let prediction_timing = &mut pipe.client_data.prediction_timing;
                let tick_time = time_until_tick(ticks_per_second);
                let predict_max = prediction_timing.pred_max_smooth();
                let ticks_in_pred = (predict_max.as_nanos() / tick_time.as_nanos()) as u64;
                let time_in_pred =
                    Duration::from_nanos((predict_max.as_nanos() % tick_time.as_nanos()) as u64);

                // we remove the overhead of the server here,
                // the reason is simple: if the server required 10ms for 63 players snapshots
                // the 64th player's client would "think" it runs 10ms behind and speeds up
                // computation, but the inputs are handled much earlier on the server then.
                let timestamp = timestamp.saturating_sub(overhead_time);
                let time_diff =
                    timestamp.as_secs_f64() - pipe.client_data.last_game_tick.as_secs_f64();
                let pred_tick = prev_tick;

                let tick_diff =
                    (pred_tick as i128 - monotonic_tick as i128) as f64 - ticks_in_pred as f64;
                let time_diff = time_diff - time_in_pred.as_secs_f64();

                let time_diff = tick_diff * tick_time.as_secs_f64() + time_diff;

                prediction_timing.add_snap(time_diff, timestamp);
            }
            ServerToClientMessage::Events {
                events,
                game_monotonic_tick,
            } => {
                let event_id = events.event_id;

                *pipe.events = events;
                pipe.map.game.sync_event_id(event_id);
            }
            ServerToClientMessage::Load(_) => {
                panic!("this should be handled by earlier logic.");
            }
            ServerToClientMessage::QueueInfo(_) => {
                // ignore
            }
            ServerToClientMessage::Chat(chat_msg) => {
                pipe.client_data.chat_msgs.push_back(chat_msg.msg);
            }
            ServerToClientMessage::InputAck { inp_id } => {
                todo!()
            }
        }
    }
}
