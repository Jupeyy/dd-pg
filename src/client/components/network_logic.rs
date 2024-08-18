use std::time::Duration;

use anyhow::anyhow;
use client_map::client_map::GameMap;
use game_interface::interface::GameStateInterface;
use pool::rc::PoolRc;
use server::server::Server;
use shared_base::{game_types::time_until_tick, network::messages::MsgClSnapshotAck};
use shared_network::messages::{MsgSvLoadVotes, ServerToClientMessage};

use crate::{
    client::component::GameMsgPipeline,
    game::SnapshotStorageItem,
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
                        pipe.game_data.prediction_timing.add_ping(
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
                let (mut snapshot, snap_id, game_monotonic_tick) = match snapshot_and_id {
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
                let monotonic_tick = game_monotonic_tick;

                let prev_tick = game.predicted_game_monotonic_tick;
                if !pipe
                    .game_data
                    .handled_snap_id
                    .is_some_and(|id| id >= snap_id)
                {
                    let local_players = game.build_from_snapshot(&snapshot);
                    // set local players
                    pipe.game_data.handle_local_players_from_snapshot(
                        pipe.config_game,
                        pipe.console_entries,
                        local_players,
                    );

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
                                snapshot: std::mem::take(&mut *snapshot.to_mut()),
                                monotonic_tick: game_monotonic_tick,
                            },
                        );
                    }
                    pipe.game_data.snap_acks.push(MsgClSnapshotAck { snap_id });

                    game.predicted_game_monotonic_tick = monotonic_tick.max(prev_tick);
                    // drop queued input that was before or at the server monotonic tick
                    while pipe
                        .game_data
                        .input_per_tick
                        .front()
                        .is_some_and(|(&tick, _)| tick < monotonic_tick)
                    {
                        pipe.game_data.input_per_tick.pop_front();
                    }
                    if prev_tick > monotonic_tick {
                        (monotonic_tick..prev_tick).for_each(|new_tick| {
                            // apply the player input if the tick had any
                            let prev_tick_of_inp = new_tick;
                            let tick_of_inp = new_tick + 1;
                            if let (Some(inp), prev_inp) = (
                                pipe.game_data.input_per_tick.get(&tick_of_inp).or_else(|| {
                                    pipe.game_data.input_per_tick.iter().rev().find_map(
                                        |(&tick, inp)| (tick <= tick_of_inp).then_some(inp),
                                    )
                                }),
                                pipe.game_data.input_per_tick.get(&prev_tick_of_inp),
                            ) {
                                inp.iter().for_each(|(player_id, player_inp)| {
                                    let mut prev_player_inp = prev_inp
                                        .or(Some(inp))
                                        .and_then(|inps| inps.get(player_id).cloned())
                                        .unwrap_or_else(Default::default);

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
                                &pipe.game_data.last_game_tick,
                                game,
                                pipe.game_data
                                    .input_per_tick
                                    .get(&tick_of_inp)
                                    .map(|inps| inps.values().map(|inp| &inp.inp)),
                                new_tick + 1,
                                ticks_per_second.get(),
                                pipe.shared_info,
                                "client-pred",
                            );
                        });
                    }
                }
                let prediction_timing = &mut pipe.game_data.prediction_timing;
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
                    timestamp.as_secs_f64() - pipe.game_data.last_game_tick.as_secs_f64();
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

                *pipe.events = Some(events);
                pipe.map.game.sync_event_id(event_id);
            }
            ServerToClientMessage::Load(_) => {
                panic!("this should be handled by earlier logic.");
            }
            ServerToClientMessage::QueueInfo(_) => {
                // ignore
            }
            ServerToClientMessage::Chat(chat_msg) => {
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
                MsgSvLoadVotes::Misc {} => todo!(),
            },
        }
    }
}
