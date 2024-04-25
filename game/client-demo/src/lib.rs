use std::{
    io::{Read, Write},
    path::Path,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

use av_encoder::encoder::AudioVideoEncoder;
use base::system::{System, SystemTimeInterface};
use base_io::{io::IO, io_batcher::IOBatcherTask};
use client_map::client_map::{ClientMapFile, ClientMapLoading, GameMap};
use client_render_base::map::render_pipe::GameStateRenderInfo;
use client_render_game::render_game::{
    RenderForPlayer, RenderGameForPlayer, RenderGameInput, RenderGameInterface,
};
use config::config::ConfigEngine;
use game_config::config::ConfigMap;
use game_interface::{
    events::{GameEvents, FIRST_EVENT_ID},
    interface::GameStateInterface,
    types::game::GameTickType,
};
use graphics::{graphics::graphics::Graphics, handles::canvas::canvas::GraphicsCanvasHandle};
use graphics_backend::backend::GraphicsBackend;
use graphics_types::commands::CommandSwitchCanvasModeType;
use pool::{datatypes::PoolLinkedHashMap, mt_datatypes::PoolVec as MtPoolVec};
use pool::{
    datatypes::{PoolVec, PoolVecDeque},
    mt_datatypes::PoolLinkedHashMap as MtPoolLinkedHashMap,
};
use serde::{Deserialize, Serialize};
use shared_base::game_types::intra_tick_time_to_ratio;
use sound::sound::SoundManager;

#[derive(Debug, Serialize, Deserialize)]
pub struct DemoHeader {
    /// length of the full demo
    pub len: Duration,
    /// optional server name, address or whatever - can be left empty
    pub server: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Demo {
    pub header: DemoHeader,
    pub physics_module: Option<Vec<u8>>,
    pub render_game_module: Option<Vec<u8>>,
    pub map: Option<Vec<u8>>,
    pub map_name: String,
    pub snapshots: Vec<Vec<u8>>,
    pub ticks_per_second: GameTickType,
}

pub struct DemoRecorder {
    pub demo: Demo,
    pub io: IO,
}

impl DemoRecorder {
    pub fn new(map_name: &str, ticks_per_second: GameTickType, io: &IO) -> Self {
        Self {
            demo: Demo {
                header: DemoHeader {
                    len: Duration::ZERO,
                    server: "".into(),
                },
                physics_module: None,
                render_game_module: None,
                map: None,
                map_name: map_name.to_string(),
                snapshots: Vec::new(),
                ticks_per_second,
            },
            io: io.clone(),
        }
    }

    pub fn add_snapshot(&mut self, snapshot: Vec<u8>) {
        self.demo.snapshots.push(snapshot);
        self.demo.header.len = Duration::from_micros(
            (1000000 / self.demo.ticks_per_second) * self.demo.snapshots.len() as u64,
        );
    }
}

impl Drop for DemoRecorder {
    fn drop(&mut self) {
        let fs = self.io.fs.clone();
        let demo = bincode::serde::encode_to_vec(&self.demo, bincode::config::standard()).unwrap();
        let _ = self.io.io_batcher.spawn_without_lifetime(async move {
            let mut demo_write: Vec<u8> = Default::default();
            brotli::CompressorWriter::new(&mut demo_write, 4096, 9, 22).write_all(&demo)?;
            fs.create_dir("demos/".as_ref()).await?;
            fs.write_file(Path::new("demos/demo.twdemo"), demo_write)
                .await?;
            Ok(())
        });
    }
}

pub struct DemoViewerImpl {
    client_map: ClientMapLoading,
    canvas_handle: GraphicsCanvasHandle,
    av_encoder: Option<(AudioVideoEncoder, Arc<Mutex<Vec<u8>>>)>,

    demo: Demo,
    last_demo_snapshot_index: usize,

    config_map: ConfigMap,

    start_time: Duration,

    io: IO,
}

impl DemoViewerImpl {
    fn new(
        client_map: ClientMapLoading,
        canvas_handle: &GraphicsCanvasHandle,
        backend: &Rc<GraphicsBackend>,
        demo: Demo,
        sys: &System,
        io: IO,
    ) -> Self {
        Self {
            client_map,
            canvas_handle: canvas_handle.clone(),
            av_encoder: Some({
                let write_file: Arc<Mutex<Vec<u8>>> = Default::default();
                (
                    AudioVideoEncoder::new(0, "test.webm", backend, &sys.log, write_file.clone()),
                    write_file,
                )
            }),

            demo,
            last_demo_snapshot_index: 0,

            config_map: Default::default(),

            start_time: sys.time_get_nanoseconds(),
            io: io.clone(),
        }
    }

    pub fn is_finished(&self) -> bool {
        self.last_demo_snapshot_index >= self.demo.snapshots.len()
    }

    pub fn render(&mut self) {
        if self.is_finished() {
            // finished demo
            return;
        }

        let map = self.client_map.try_get_mut().unwrap();

        let ClientMapFile::Game(GameMap { render, game }) = map else {
            panic!("not a game map")
        };

        let snap = &self.demo.snapshots[self.last_demo_snapshot_index];
        self.last_demo_snapshot_index += 1;
        let mut snapshot = MtPoolVec::new_without_pool();
        snapshot.extend(snap.iter());
        let local_players = game.build_from_snapshot(&snapshot);
        game.pred_tick(PoolLinkedHashMap::new_without_pool()); // TODO:

        let cur_time = self.start_time
            + Duration::from_micros(
                (self.last_demo_snapshot_index as u64 * 1000000) / self.demo.ticks_per_second,
            );

        let (player_id, _) = local_players.iter().next().unwrap();
        let intra_tick_time = Duration::ZERO;
        let ticks_per_second = self.demo.ticks_per_second;

        let render_for_player = RenderForPlayer {
            chat_info: None,
            scoreboard_active: false,

            local_player_info: game.collect_character_local_render_info(player_id),

            player_id: *player_id,
        };

        let game_info = GameStateRenderInfo {
            ticks_per_second: game.game_tick_speed(),
            intra_tick_time,
        };
        let intra_tick_ratio = intra_tick_time_to_ratio(intra_tick_time, ticks_per_second);

        let player_render_infos = game.collect_characters_render_info(intra_tick_time_to_ratio(
            intra_tick_time,
            ticks_per_second,
        ));
        let character_infos = game.collect_characters_info();

        let projectiles = game.all_projectiles(intra_tick_ratio);
        let flags = game.all_ctf_flags(intra_tick_ratio);
        let lasers = game.all_lasers(intra_tick_ratio);
        let pickups = game.all_pickups(intra_tick_ratio);

        let scoreboard_info = game.collect_scoreboard_info();

        let chat_msgs = PoolVecDeque::new_without_pool();
        let mut render_game_input = RenderGameInput {
            players: PoolVec::new_without_pool(),
            events: GameEvents {
                worlds: MtPoolLinkedHashMap::new_without_pool(),
                event_id: FIRST_EVENT_ID,
            },
            character_render_infos: player_render_infos,
            character_infos,
            projectiles,
            flags,
            lasers,
            pickups,
            scoreboard_info,
            chat_msgs,
        };

        let render_for_player = RenderGameForPlayer {
            render_for_player: Some(render_for_player),
            game_state_info: game_info,
        };

        render_game_input.players.push(render_for_player);

        self.canvas_handle
            .switch_canvas(CommandSwitchCanvasModeType::Offscreen {
                id: 0,
                width: 800,
                height: 600,
                has_multi_sampling: None,
            });

        render.render(&self.config_map, &cur_time, render_game_input);
        self.canvas_handle
            .switch_canvas(CommandSwitchCanvasModeType::Onscreen);
    }
}

impl Drop for DemoViewerImpl {
    fn drop(&mut self) {
        if let Some((encoder, file)) = self.av_encoder.take() {
            drop(encoder);
            let file = Arc::into_inner(file).unwrap().into_inner().unwrap();
            let fs = self.io.fs.clone();
            let _ = self.io.io_batcher.spawn_without_lifetime(async move {
                fs.write_file("testtest.webm".as_ref(), file).await?;
                Ok(())
            });
        }
    }
}

pub enum DemoViewer {
    Loading {
        task: IOBatcherTask<Vec<u8>>,
        io: IO,
        thread_pool: Arc<rayon::ThreadPool>,
    },
    LoadingComponents {
        client_map: ClientMapLoading,
        demo: Demo,
        io: IO,
    },
    Rendering(DemoViewerImpl),
    None,
}

impl DemoViewer {
    pub fn new(io: &IO, thread_pool: &Arc<rayon::ThreadPool>, demo_name: String) -> Self {
        let fs = io.fs.clone();
        let demo_name = demo_name.to_string();
        let read_demo = io.io_batcher.spawn(async move {
            let demo = fs
                .open_file(format!("demos/{}", demo_name).as_ref())
                .await?;
            let mut demo_read: Vec<u8> = Default::default();
            brotli::CompressorReader::new(demo.as_slice(), 4096, 9, 22)
                .read_to_end(&mut demo_read)?;
            Ok(demo_read)
        });
        Self::Loading {
            task: read_demo,
            io: io.clone(),
            thread_pool: thread_pool.clone(),
        }
    }

    pub fn try_get(&self) -> Option<&DemoViewerImpl> {
        if let Self::Rendering(viewer) = self {
            Some(viewer)
        } else {
            None
        }
    }

    pub fn try_get_mut(&mut self) -> Option<&mut DemoViewerImpl> {
        if let Self::Rendering(viewer) = self {
            Some(viewer)
        } else {
            None
        }
    }

    pub fn continue_loading(
        &mut self,
        sound: &SoundManager,
        graphics: &Graphics,
        backend: &Rc<GraphicsBackend>,
        config: &ConfigEngine,
        sys: &System,
    ) -> anyhow::Result<Option<&DemoViewerImpl>> {
        let mut dummy = DemoViewer::None;
        std::mem::swap(self, &mut dummy);
        match dummy {
            DemoViewer::Loading {
                task,
                io,
                thread_pool,
            } => {
                if task.is_finished() {
                    let demo_file = task.get_storage()?;
                    let demo = bincode::serde::decode_from_slice::<Demo, _>(
                        &demo_file,
                        bincode::config::standard(),
                    )?
                    .0;
                    *self = Self::LoadingComponents {
                        client_map: ClientMapLoading::new(
                            &demo.map_name,
                            None,
                            None,
                            &io,
                            &thread_pool,
                            false,
                        ),
                        demo,
                        io,
                    };
                } else {
                    *self = Self::Loading {
                        task,
                        io,
                        thread_pool,
                    };
                }
            }
            DemoViewer::LoadingComponents {
                mut client_map,
                demo,
                io,
            } => {
                if client_map
                    .continue_loading(sound, graphics, backend, config, sys)
                    .is_some()
                {
                    // finished loading client
                    *self = Self::Rendering(DemoViewerImpl::new(
                        client_map,
                        &graphics.canvas_handle,
                        backend,
                        demo,
                        sys,
                        io,
                    ));
                } else {
                    *self = Self::LoadingComponents {
                        client_map,
                        demo,
                        io,
                    };
                }
            }
            DemoViewer::Rendering(viewer) => {
                *self = Self::Rendering(viewer);
            }
            DemoViewer::None => {}
        }
        Ok(self.try_get())
    }
}
