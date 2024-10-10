use std::{
    collections::{BTreeMap, VecDeque},
    time::Duration,
};

use binds::binds::{
    gen_local_player_action_hash_map, syn_to_bind, BindActions, BindActionsLocalPlayer,
};
use client_types::console::{entries_to_parser, ConsoleEntry};
use command_parser::parser::{self, CommandType};
use game_config::config::ConfigGame;
use game_interface::{
    types::{
        game::{GameEntityId, GameTickType},
        input::CharacterInputInfo,
        snapshot::SnapshotLocalPlayers,
        weapons::WeaponType,
    },
    votes::{MapVote, MiscVote, VoteState, Voted},
};
use hashlink::{LinkedHashMap, LinkedHashSet};
use math::math::vector::luffixed;
use native::{
    input::binds::{BindKey, MouseExtra},
    native::{KeyCode, MouseButton, PhysicalKey},
};
use pool::{datatypes::PoolVecDeque, mt_datatypes::PoolCow, pool::Pool, rc::PoolRc};
use prediction_timer::prediction_timing::PredictionTimer;
use shared_base::{
    network::{messages::MsgClSnapshotAck, types::chat::NetChatMsg},
    player_input::PlayerInput,
};

use crate::{
    client::input::input_handling::DeviceToLocalPlayerIndex,
    localplayer::{ClientPlayer, ClientPlayerInputPerTick, LocalPlayers},
};

#[derive(Debug)]
pub struct SnapshotStorageItem {
    pub snapshot: Vec<u8>,
    pub monotonic_tick: u64,
}

#[derive(Debug, Default)]
pub struct NetworkByteStats {
    pub last_timestamp: Duration,
    pub last_bytes_sent: u64,
    pub last_bytes_recv: u64,
    pub bytes_per_sec_sent: luffixed,
    pub bytes_per_sec_recv: luffixed,
}

pub struct GameData {
    pub local_players: LocalPlayers,

    /// Snapshot that still has to be acknowledged.
    pub snap_acks: Vec<MsgClSnapshotAck>,

    pub device_to_local_player_index: DeviceToLocalPlayerIndex, // TODO: keyboard and mouse are different devices
    pub input_per_tick: ClientPlayerInputPerTick,

    /// This is only used to make sure old snapshots are not handled.
    pub handled_snap_id: Option<u64>,
    pub last_snap: Option<(PoolCow<'static, [u8]>, GameTickType)>,

    /// Only interesting for future tick prediction
    pub cur_state_snap: Option<PoolCow<'static, [u8]>>,

    /// Ever increasing id for sending input packages.
    pub input_id: u64,

    /// last (few) snapshot diffs & id client used
    pub snap_storage: BTreeMap<u64, SnapshotStorageItem>,

    /// A tracker of sent inputs and their time
    /// used to evaluate the estimated RTT/ping.
    pub sent_input_ids: BTreeMap<u64, Duration>,

    pub prediction_timer: PredictionTimer,
    pub net_byte_stats: NetworkByteStats,

    pub last_game_tick: Duration,
    pub last_frame_time: Duration,
    pub intra_tick_time: Duration,

    pub chat_msgs_pool: Pool<VecDeque<NetChatMsg>>,
    pub chat_msgs: PoolVecDeque<NetChatMsg>,
    pub player_inp_pool: Pool<LinkedHashMap<GameEntityId, PlayerInput>>,
    pub player_snap_pool: Pool<Vec<u8>>,
    pub player_inputs_state_pool: Pool<LinkedHashMap<GameEntityId, CharacterInputInfo>>,
    pub player_ids_pool: Pool<LinkedHashSet<GameEntityId>>,

    /// current vote in the game and the network timestamp when it arrived
    pub vote: Option<(PoolRc<VoteState>, Option<Voted>, Duration)>,

    pub map_votes: Vec<MapVote>,
    pub misc_votes: Vec<MiscVote>,
}

impl GameData {
    pub fn new(cur_time: Duration, prediction_timer: PredictionTimer) -> Self {
        let chat_and_system_msgs_pool = Pool::with_capacity(2);
        Self {
            local_players: LocalPlayers::new(),

            snap_acks: Vec::with_capacity(16),

            input_id: 0,
            last_snap: None,

            cur_state_snap: None,

            snap_storage: Default::default(),

            device_to_local_player_index: Default::default(),
            input_per_tick: Default::default(),

            sent_input_ids: Default::default(),

            handled_snap_id: None,
            prediction_timer,
            net_byte_stats: Default::default(),

            last_game_tick: cur_time,
            intra_tick_time: Duration::ZERO,
            last_frame_time: cur_time,

            chat_msgs: chat_and_system_msgs_pool.new(),
            chat_msgs_pool: chat_and_system_msgs_pool,
            player_inp_pool: Pool::with_capacity(64),
            player_snap_pool: Pool::with_capacity(2),
            player_inputs_state_pool: Pool::with_capacity(2),
            player_ids_pool: Pool::with_capacity(4),

            vote: None,
            map_votes: Default::default(),
            misc_votes: Default::default(),
        }
    }
}

impl GameData {
    pub fn handle_local_players_from_snapshot(
        local_players: &mut LocalPlayers,
        config: &ConfigGame,
        console_entries: &[ConsoleEntry],
        snap_local_players: &SnapshotLocalPlayers,
    ) {
        local_players.retain_with_order(|player_id, _| snap_local_players.contains_key(player_id));
        snap_local_players.iter().for_each(|(id, snap_player)| {
            if !local_players.contains_key(id) {
                let mut local_player: ClientPlayer = ClientPlayer {
                    is_dummy: snap_player.is_dummy,
                    zoom: 1.0,
                    ..Default::default()
                };
                let binds = &mut local_player.binds;
                let map = gen_local_player_action_hash_map();

                if snap_player.is_dummy {
                    if let Some((player, dummy)) = config
                        .players
                        .get(config.profiles.main as usize)
                        .zip(config.players.get(config.profiles.dummy.index as usize))
                    {
                        let bind_player = if config.profiles.dummy.copy_binds_from_main {
                            player
                        } else {
                            dummy
                        };
                        for bind in &bind_player.binds {
                            let cmds = parser::parse(bind, &entries_to_parser(console_entries));
                            for cmd in &cmds {
                                if let CommandType::Full(cmd) = cmd {
                                    let (keys, actions) = syn_to_bind(&cmd.args, &map).unwrap();

                                    binds.register_bind(&keys, actions);
                                }
                            }
                        }
                    }
                } else if let Some(player) = config.players.get(config.profiles.main as usize) {
                    for bind in &player.binds {
                        let cmds = parser::parse(bind, &entries_to_parser(console_entries));
                        for cmd in &cmds {
                            if let CommandType::Full(cmd) = cmd {
                                let (keys, actions) = syn_to_bind(&cmd.args, &map).unwrap();

                                binds.register_bind(&keys, actions);
                            }
                        }
                    }
                }

                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::KeyA))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::MoveLeft)],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::KeyD))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::MoveRight)],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::Space))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::Jump)],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::Escape))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::OpenMenu)],
                );
                binds.register_bind(
                    &[BindKey::Mouse(MouseButton::Left)],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::Fire)],
                );
                binds.register_bind(
                    &[BindKey::Mouse(MouseButton::Right)],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::Hook)],
                );
                binds.register_bind(
                    &[BindKey::Extra(MouseExtra::WheelDown)],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::PrevWeapon)],
                );
                binds.register_bind(
                    &[BindKey::Extra(MouseExtra::WheelUp)],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::NextWeapon)],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::Digit1))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::Weapon(
                        WeaponType::Hammer,
                    ))],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::Digit2))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::Weapon(
                        WeaponType::Gun,
                    ))],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::Digit3))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::Weapon(
                        WeaponType::Shotgun,
                    ))],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::Digit4))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::Weapon(
                        WeaponType::Grenade,
                    ))],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::Digit5))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::Weapon(
                        WeaponType::Laser,
                    ))],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::KeyG))],
                    vec![BindActions::LocalPlayer(
                        BindActionsLocalPlayer::ToggleDummyCopyMoves,
                    )],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::Enter))],
                    vec![BindActions::LocalPlayer(
                        BindActionsLocalPlayer::ActivateChatInput,
                    )],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::KeyT))],
                    vec![BindActions::LocalPlayer(
                        BindActionsLocalPlayer::ActivateChatInput,
                    )],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::Tab))],
                    vec![BindActions::LocalPlayer(
                        BindActionsLocalPlayer::ShowScoreboard,
                    )],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::KeyU))],
                    vec![BindActions::LocalPlayer(
                        BindActionsLocalPlayer::ShowChatHistory,
                    )],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::ShiftLeft))],
                    vec![BindActions::LocalPlayer(
                        BindActionsLocalPlayer::ShowEmoteWheel,
                    )],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::KeyQ))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::Kill)],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::F3))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::VoteYes)],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::F4))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::VoteNo)],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::NumpadSubtract))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::ZoomOut)],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::NumpadAdd))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::ZoomIn)],
                );
                binds.register_bind(
                    &[BindKey::Key(PhysicalKey::Code(KeyCode::NumpadMultiply))],
                    vec![BindActions::LocalPlayer(BindActionsLocalPlayer::ZoomReset)],
                );
                local_players.insert(*id, local_player);
            }
            // sort
            if let Some(local_player) = local_players.to_back(id) {
                local_player.input_cam_mode = snap_player.input_cam_mode;
            }
        });
    }
}
