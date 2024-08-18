use std::{
    cell::Cell,
    collections::{BTreeMap, HashMap},
    fs::File,
    io::{Read, Seek, Write},
    ops::RangeBounds,
    path::Path,
    rc::Rc,
    sync::{
        mpsc::{Receiver, Sender},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::Duration,
};

use anyhow::anyhow;
use av_encoder::encoder::AudioVideoEncoder;
use base::{
    hash::Hash,
    system::{System, SystemTime, SystemTimeInterface},
};
use base_io::{io::Io, io_batcher::IoBatcherTask};
use client_map::client_map::{ClientMapFile, ClientMapLoading, GameMap};
use client_render_base::map::render_pipe::GameTimeInfo;
use client_render_game::render_game::{
    RenderForPlayer, RenderGameForPlayer, RenderGameInput, RenderGameInterface,
};
use config::config::ConfigEngine;
use game_config::config::ConfigMap;
use game_interface::{
    events::GameEvents,
    interface::{GameStateCreateOptions, GameStateInterface},
    types::{game::NonZeroGameTickType, reduced_ascii_str::ReducedAsciiString},
};
use graphics::{graphics::graphics::Graphics, handles::canvas::canvas::GraphicsCanvasHandle};
use graphics_backend::backend::GraphicsBackend;
use graphics_types::commands::CommandSwitchCanvasModeType;
use itertools::Itertools;
use pool::datatypes::{PoolVec, PoolVecDeque};
use pool::mt_datatypes::PoolCow as MtPoolCow;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use shared_base::network::messages::GameModification;
use sound::sound::SoundManager;
use ui_base::font_data::UiFontData;

pub type DemoGameModification = GameModification;

/// The demo header, of const size.
/// A broken demo can be detected if [`DemoHeader::len`] or
/// [`DemoHeader::size_chunks`] is zero.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct DemoHeader {
    /// Length of the full demo
    pub len: Duration,
    /// Size to read for the whole [`DemoHeaderExt`] struct.
    pub size_ext: u64,
    /// Size to read for all chunks.
    pub size_chunks: u64,
}

/// The tail of the demo is written last,
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DemoTail {
    /// the key is the monotonic tick, while the value is the
    /// file offset relative to the beginning of the chunks.
    index: BTreeMap<u64, u64>,
}

/// A more flexible header, that can contain dynamic sized elements.
/// Here header simply means, never changing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemoHeaderExt {
    /// optional server name, address or whatever - can be left empty
    pub server: String,
    pub physics_mod: DemoGameModification,
    pub render_mod: DemoGameModification,
    /// resources the game **has** to load before
    /// the game/demo starts (e.g. because the game mod requires
    /// them for gameplay).
    pub required_resources: HashMap<String, String>,
    pub map: ReducedAsciiString,
    pub map_hash: Hash,
    pub ticks_per_second: NonZeroGameTickType,
    pub game_options: GameStateCreateOptions,
}

/// When a [`DemoChunk`] is serialized, this header
/// is written.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChunkHeader {
    monotonic_tick: u64,
    snap: u64,
    evs: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct DemoChunk {
    pub snapshot: Option<Vec<u8>>,
    pub events: Option<GameEvents>,
}

enum DemoRecorderEvents {
    Chunks { chunks: BTreeMap<u64, DemoChunk> },
}

/// Records demos from snapshots & events
pub struct DemoRecorder {
    /// The dynamic sized header is only written once
    pub demo_header_ext: DemoHeaderExt,
    /// current demo chunks
    pub demo_chunks: BTreeMap<u64, DemoChunk>,

    /// Event sender for the writer thread.
    /// Must stay to not be dropped
    thread_sender: Sender<DemoRecorderEvents>,
    /// the thread that writes all demo changes to disk
    _writer_thread: JoinHandle<()>,
}

// 50 here is the assumed default tick rate
// so it writes up to 30 seconds full of chunks
/// number of chunks to write at once
const CHUNKS_TO_WRITE: u64 = 30 * 50;
/// time offset so that even late packets have a chance
/// to be considered in the demo.
const SECONDS_UNTIL_WRITE: u64 = 3;

#[derive(Debug, Clone)]
pub struct DemoRecorderCreateProps {
    pub map: ReducedAsciiString,
    pub map_hash: Hash,
    pub game_options: GameStateCreateOptions,
    pub required_resources: HashMap<String, String>,
    pub physics_module: DemoGameModification,
    pub render_module: DemoGameModification,
    pub io: Io,
}

impl DemoRecorder {
    pub fn new(props: DemoRecorderCreateProps, ticks_per_second: NonZeroGameTickType) -> Self {
        let (thread_sender, recv) = std::sync::mpsc::channel();

        let demo_header_ext = DemoHeaderExt {
            server: "".into(),
            physics_mod: props.physics_module,
            render_mod: props.render_module,
            required_resources: props.required_resources,
            map: props.map,
            map_hash: props.map_hash,
            ticks_per_second,
            game_options: props.game_options,
        };

        let io = props.io;

        let tmp_demo_dir = io.fs.get_save_path().join("tmp/demos");
        let demo_dir = io.fs.get_save_path().join("demos");
        let demo_header_ext_thread = demo_header_ext.clone();
        let writer_thread = std::thread::Builder::new()
            .name(format!("demo-recorder-{}", demo_header_ext.map.as_str()))
            .spawn(move || {
                Self::writer_thread_run(&tmp_demo_dir, &demo_dir, recv, demo_header_ext_thread)
                    .unwrap()
            })
            .expect("could not spawn a demo-recorder thread.");

        Self {
            demo_header_ext,
            demo_chunks: Default::default(),

            thread_sender,
            _writer_thread: writer_thread,
        }
    }

    fn writer_thread_run(
        tmp_path: &Path,
        final_path: &Path,
        recv: Receiver<DemoRecorderEvents>,
        header_ext: DemoHeaderExt,
    ) -> anyhow::Result<()> {
        std::fs::create_dir_all(tmp_path)?;
        std::fs::create_dir_all(final_path)?;
        let mut tmp_file = tempfile::NamedTempFile::new_in(tmp_path)?;
        let file = tmp_file.as_file_mut();
        let size = Cell::new(0);

        fn ser_ex<'a, T: Serialize>(
            v: &T,
            writer: &'a mut Vec<u8>,
            clear: bool,
            fixed_size: bool,
        ) -> anyhow::Result<&'a mut [u8]> {
            if clear {
                writer.clear();
            }
            let config = bincode::config::standard();
            if fixed_size {
                bincode::serde::encode_into_std_write(v, writer, config.with_fixed_int_encoding())?;
            } else {
                bincode::serde::encode_into_std_write(v, writer, config)?;
            }
            Ok(writer.as_mut_slice())
        }
        fn ser<'a, T: Serialize>(v: &T, writer: &'a mut Vec<u8>) -> anyhow::Result<&'a mut [u8]> {
            ser_ex(v, writer, true, false)
        }

        fn comp<'a>(v: &[u8], writer: &'a mut Vec<u8>) -> anyhow::Result<&'a mut [u8]> {
            writer.clear();
            let mut encoder = zstd::Encoder::new(&mut *writer, 0)?;
            encoder.write_all(v)?;
            encoder.finish()?;
            Ok(writer.as_mut_slice())
        }

        let write = |file: &mut File, v: &[u8]| -> anyhow::Result<()> {
            size.set(size.get() + v.len());
            Ok(file.write_all(v)?)
        };

        let mut write_ser = Vec::new();
        let mut write_comp = Vec::new();
        let mut write_dst = Vec::new();
        let mut write_snap = Vec::new();
        let mut write_evs = Vec::new();

        let header_ext_file = comp(ser(&header_ext, &mut write_ser)?, &mut write_comp)?;
        let header_ext_len = header_ext_file.len();

        write(
            &mut *file,
            ser_ex(
                &DemoHeader {
                    len: Duration::ZERO,
                    size_ext: header_ext_len as u64,
                    // don't update this value before ending the demo
                    // makes it easy to detect corrupted demos
                    size_chunks: 0,
                },
                &mut write_ser,
                true,
                true,
            )?,
        )?;

        write(&mut *file, header_ext_file)?;

        let mut first_monotonic = None;
        let mut last_monotonic = None;

        let mut index: BTreeMap<u64, u64> = Default::default();

        let size_before_chunks = size.get();

        fn write_chunks<'a>(
            chunks: BTreeMap<u64, DemoChunk>,
            writer: &'a mut Vec<u8>,
            tmp: &mut Vec<u8>,
            tmp_dst: &mut Vec<u8>,
            tmp_patch_snap: &mut Vec<u8>,
            tmp_patch_evs: &mut Vec<u8>,
        ) -> anyhow::Result<&'a [u8]> {
            writer.clear();

            let mut last_snap: Option<Vec<u8>> = None;
            let mut last_event: Option<Vec<u8>> = None;

            // first write chunk count
            let len_ser = ser(&(chunks.len() as u64), &mut *tmp)?;
            writer.write_all(len_ser)?;

            for (monotonic_tick, chunk) in chunks {
                tmp_patch_snap.clear();
                tmp_patch_evs.clear();

                // prepare optimized snaps & events
                let snapshot = if let Some(snapshot) = &chunk.snapshot {
                    let snap = ser(snapshot, tmp_dst)?;
                    let snapshot = if let Some(last_snap) = &last_snap {
                        bin_patch::diff(last_snap, snap, &mut *tmp_patch_snap)?;
                        Some(tmp_patch_snap.as_mut_slice())
                    } else {
                        Some(comp(snap, tmp_patch_snap)?)
                    };
                    last_snap = Some(snap.to_vec());
                    snapshot
                } else {
                    None
                };
                let events = if let Some(events) = &chunk.events {
                    let evs = ser(events, tmp_dst)?;
                    let events = if let Some(last_event) = &last_event {
                        bin_patch::diff(last_event, evs, &mut *tmp_patch_evs)?;
                        Some(tmp_patch_evs.as_mut_slice())
                    } else {
                        Some(comp(evs, tmp_patch_evs)?)
                    };
                    last_event = Some(evs.to_vec());
                    events
                } else {
                    None
                };

                let mono_ser = ser(
                    &ChunkHeader {
                        monotonic_tick,
                        snap: snapshot
                            .as_ref()
                            .map(|s| s.len() as u64)
                            .unwrap_or_default(),
                        evs: events.as_ref().map(|e| e.len() as u64).unwrap_or_default(),
                    },
                    &mut *tmp,
                )?;
                writer.write_all(mono_ser)?;
                // now write the snaps & events
                if let Some(snapshot) = snapshot {
                    writer.write_all(snapshot)?;
                }
                if let Some(events) = events {
                    writer.write_all(events)?;
                }
            }

            Ok(writer.as_mut_slice())
        }

        while let Ok(event) = recv.recv() {
            match event {
                DemoRecorderEvents::Chunks { chunks } => {
                    let first_tick = chunks
                        .first_key_value()
                        .map(|(c, _)| *c)
                        .ok_or_else(|| anyhow!("empty chunks are not allowed."))?;

                    let last_tick = chunks
                        .last_key_value()
                        .map(|(c, _)| *c)
                        .ok_or_else(|| anyhow!("empty chunks are not allowed."))?;

                    index.insert(first_tick, (size.get() - size_before_chunks) as u64);

                    write(
                        &mut *file,
                        write_chunks(
                            chunks,
                            &mut write_ser,
                            &mut write_comp,
                            &mut write_dst,
                            &mut write_snap,
                            &mut write_evs,
                        )?,
                    )?;

                    let first_tick = *first_monotonic.get_or_insert(first_tick);
                    let last_tick = *last_monotonic.insert(last_tick);
                    anyhow::ensure!(
                        last_tick >= first_tick,
                        "somehow the first monotonic tick was bigger than the current last one."
                    );
                }
            }
        }

        let chunks_size = size.get() - size_before_chunks;

        if let Some((first_monotonic, last_monotonic)) = first_monotonic.zip(last_monotonic) {
            // write the demo tail
            write(
                &mut *file,
                comp(ser(&DemoTail { index }, &mut write_ser)?, &mut write_comp)?,
            )?;

            // write the final header
            file.seek(std::io::SeekFrom::Start(0))?;
            file.write_all(ser_ex(
                &DemoHeader {
                    len: {
                        let secs = (last_monotonic - first_monotonic) / header_ext.ticks_per_second;
                        let nanos = ((last_monotonic - first_monotonic)
                            % header_ext.ticks_per_second)
                            * (Duration::from_secs(1).as_nanos() as u64
                                / header_ext.ticks_per_second);
                        Duration::new(secs, nanos as u32)
                    },
                    size_ext: header_ext_len as u64,
                    size_chunks: chunks_size as u64,
                },
                &mut write_ser,
                true,
                true,
            )?)?;

            file.flush()?;

            let (_, path) = tmp_file.keep()?;
            std::fs::rename(path, final_path.join("demo.twdemo"))?;
        }
        // else the demo is invalid and can be dropped.

        Ok(())
    }

    fn try_write_chunks(&mut self) {
        let get_chunks_th = || {
            let count = self.demo_chunks.len();
            // find key as fast as possible
            if count >= (CHUNKS_TO_WRITE * 2) as usize {
                self.demo_chunks.keys().nth(CHUNKS_TO_WRITE as usize)
            } else {
                self.demo_chunks
                    .keys()
                    .rev()
                    .nth((count - 1) - CHUNKS_TO_WRITE as usize)
            }
        };

        // check if chunks should be flushed
        // and the difference is > 3 seconds
        if self.demo_chunks.len() > CHUNKS_TO_WRITE as usize
            && get_chunks_th()
                .copied()
                .zip(self.demo_chunks.last_key_value().map(|(&tick, _)| tick))
                .is_some_and(|(first, last)| {
                    last - first > self.demo_header_ext.ticks_per_second.get() * SECONDS_UNTIL_WRITE
                })
        {
            let key = get_chunks_th();

            if let Some(&key) = key {
                let chunks = self.demo_chunks.split_off(&key);
                // bit overcomplicated but split_off :/
                let chunks = std::mem::replace(&mut self.demo_chunks, chunks);

                // ignore the error here, if the write thread died, so be it, can't recover anyway.
                let _ = self
                    .thread_sender
                    .send(DemoRecorderEvents::Chunks { chunks });
            }
        }
    }

    pub fn add_snapshot(&mut self, monotonic_tick: u64, snapshot: Vec<u8>) {
        self.try_write_chunks();

        // make sure only snapshots of the last 3 seconds are handled
        if self.demo_chunks.is_empty()
            || self.demo_chunks.last_key_value().is_some_and(|(&key, _)| {
                monotonic_tick >= key
                    || (key - monotonic_tick)
                        <= self.demo_header_ext.ticks_per_second.get() * SECONDS_UNTIL_WRITE
            })
        {
            // if the entry already exist, update if, else create a new
            let entry = self.demo_chunks.entry(monotonic_tick).or_default();

            entry.snapshot = Some(snapshot);
        }
    }
}

impl Drop for DemoRecorder {
    fn drop(&mut self) {
        // write remaining chunks
        if !self.demo_chunks.is_empty() {
            std::mem::take(&mut self.demo_chunks)
                .into_iter()
                .chunks(CHUNKS_TO_WRITE as usize)
                .into_iter()
                .map(|chunks| chunks.collect::<BTreeMap<_, _>>())
                .filter(|c| !c.is_empty())
                .for_each(|chunks| {
                    // ignore the error here, if the write thread died, so be it, can't recover anyway.
                    let _ = self
                        .thread_sender
                        .send(DemoRecorderEvents::Chunks { chunks });
                });
        }
    }
}

#[derive(Debug)]
pub struct DemoContainer {
    /// The const sized header
    pub header: DemoHeader,
    /// Dynamic sized header
    pub header_ext: DemoHeaderExt,
    /// Demo chunks, still untouched (compressed, serialized)
    pub demo_chunks: Vec<u8>,
    pub tail: DemoTail,
}

pub struct DemoViewerInner {
    demo: DemoContainer,
    cur_chunks: BTreeMap<u64, DemoChunk>,
    cur_time: Duration,
}

impl DemoViewerInner {
    pub fn is_finished(&self) -> bool {
        self.cur_time >= self.demo.header.len
    }

    fn read_chunks(&self, offset: usize) -> anyhow::Result<BTreeMap<u64, DemoChunk>> {
        let file = &self.demo.demo_chunks[offset..];

        fn deser<T: DeserializeOwned>(v: &[u8]) -> anyhow::Result<(T, usize)> {
            Ok(bincode::serde::decode_from_slice(
                v,
                bincode::config::standard(),
            )?)
        }
        fn decomp<'a>(v: &[u8], writer: &'a mut Vec<u8>) -> anyhow::Result<&'a [u8]> {
            writer.clear();
            let mut decoder = zstd::Decoder::new(v)?;
            decoder.read_to_end(&mut *writer)?;
            decoder.finish();

            Ok(writer.as_mut_slice())
        }
        let (len, read_size) = deser::<u64>(file)?;

        let mut file = &file[read_size..];

        let mut res: BTreeMap<u64, DemoChunk> = Default::default();

        let mut last_snap: Option<Vec<u8>> = None;
        let mut last_events: Option<Vec<u8>> = None;

        let mut writer: Vec<u8> = Default::default();

        for _ in 0..len {
            let (header, read_size) = deser::<ChunkHeader>(file)?;
            file = &file[read_size..];

            let snap = if header.snap > 0 {
                let snap = &file[..header.snap as usize];
                let res = if let Some(last_snap) = &last_snap {
                    writer.clear();
                    bin_patch::patch(last_snap, snap, &mut writer)?;
                    writer.clone()
                } else {
                    decomp(snap, &mut writer)?;
                    writer.clone()
                };
                last_snap = Some(res.clone());
                let (snap, _) = deser(&res)?;
                file = &file[header.snap as usize..];
                Some(snap)
            } else {
                None
            };
            let evs = if header.evs > 0 {
                let evs = &file[..header.evs as usize];
                let res = if let Some(last_evs) = &last_events {
                    writer.clear();
                    bin_patch::patch(last_evs, evs, &mut writer)?;
                    writer.clone()
                } else {
                    decomp(evs, &mut writer)?;
                    writer.clone()
                };
                last_events = Some(res.clone());
                let (evs, _) = deser(&res)?;
                file = &file[header.evs as usize..];
                Some(evs)
            } else {
                None
            };

            let chunk = DemoChunk {
                snapshot: snap,
                events: evs,
            };

            res.insert(header.monotonic_tick, chunk);
        }

        Ok(res)
    }

    fn time_to_tick(&self) -> u64 {
        (self.cur_time.as_nanos()
            / (Duration::from_secs(1).as_nanos()
                / self.demo.header_ext.ticks_per_second.get() as u128)) as u64
            + self.demo.tail.index.first_key_value().unwrap().0
    }

    fn intra_tick_time(&self) -> Duration {
        let time_in_tick = self.cur_time.as_nanos()
            % (Duration::from_secs(1).as_nanos()
                / self.demo.header_ext.ticks_per_second.get() as u128);
        Duration::from_nanos(time_in_tick as u64)
    }

    fn intra_tick_ratio(&self, prev_monotonic_tick: u64, next_monotonic_tick: u64) -> f64 {
        let nanos_in_tick =
            Duration::from_secs(1).as_nanos() / self.demo.header_ext.ticks_per_second.get() as u128;
        let time_tick = (self.time_to_tick() - prev_monotonic_tick) as u128 * nanos_in_tick;
        let time_range = (next_monotonic_tick - prev_monotonic_tick) as u128 * nanos_in_tick;
        // prevent division by 0
        if time_range == 0 {
            0.0
        } else {
            time_tick as f64 / time_range as f64
        }
    }

    fn try_load_chunks<R: RangeBounds<u64>>(&mut self, tick_range: R, rev: bool) {
        let mut it = self.demo.tail.index.range(tick_range);
        if let Some((_, chunk_byte_offset)) = if rev { it.next_back() } else { it.next() } {
            if let Ok(mut chunks) = self.read_chunks(*chunk_byte_offset as usize) {
                self.cur_chunks.append(&mut chunks);
            }
        }
    }

    fn check_chunks(&mut self, monotonic_tick: u64) {
        // we want exactly one chunk before the current tick
        // this allows the second chunks to be used for the tick
        // after this one.
        while self.cur_chunks.range(0..=monotonic_tick).count() > 1 {
            self.cur_chunks.pop_first();
        }

        // try to load chunks if needed
        let first_tick = self
            .cur_chunks
            .first_key_value()
            .map_or(u64::MAX, |(&tick, _)| tick);
        if first_tick > monotonic_tick {
            self.try_load_chunks(..=monotonic_tick, true)
        }
        let last_tick = self
            .cur_chunks
            .last_key_value()
            .map_or(0, |(&tick, _)| tick);
        if last_tick < monotonic_tick + 1 {
            self.try_load_chunks(last_tick + 1.., false)
        }
    }
}

pub struct DemoViewerImpl {
    client_map: ClientMapLoading,
    canvas_handle: GraphicsCanvasHandle,
    av_encoder: Option<(AudioVideoEncoder, Arc<Mutex<Vec<u8>>>)>,

    config_map: ConfigMap,

    inner: DemoViewerInner,

    io: Io,
    time: Arc<SystemTime>,

    last_time: Option<Duration>,
}

impl DemoViewerImpl {
    fn new(
        client_map: ClientMapLoading,
        canvas_handle: &GraphicsCanvasHandle,
        backend: &Rc<GraphicsBackend>,
        demo: DemoContainer,
        sys: &System,
        io: Io,
    ) -> Self {
        Self {
            client_map,
            canvas_handle: canvas_handle.clone(),
            av_encoder: None, /* TODO: Some({
                                  let write_file: Arc<Mutex<Vec<u8>>> = Default::default();
                                  (
                                      AudioVideoEncoder::new(0, "test.webm", backend, &sys.log, write_file.clone()),
                                      write_file,
                                  )
                              })*/

            io: io.clone(),
            time: sys.time.clone(),
            config_map: Default::default(),
            last_time: None,

            inner: DemoViewerInner {
                demo,

                cur_chunks: Default::default(),

                cur_time: Duration::ZERO,
            },
        }
    }

    pub fn is_finished(&self) -> bool {
        self.inner.is_finished()
    }

    pub fn render(&mut self) -> anyhow::Result<()> {
        if self.is_finished() {
            // finished demo
            return Ok(());
        }

        let cur_time = self.time.time_get_nanoseconds();
        let last_time = self.last_time.replace(cur_time).unwrap_or(cur_time);
        self.inner.cur_time += cur_time.saturating_sub(last_time);

        let monotonic_tick = self.inner.time_to_tick();

        self.inner.check_chunks(monotonic_tick);

        let map = self.client_map.try_get_mut().unwrap();

        let ClientMapFile::Game(GameMap { render, game }) = map else {
            panic!("not a game map")
        };

        let Some((local_players, prev_tick, next_tick)) = (if !self.inner.cur_chunks.is_empty() {
            let mut it = self.inner.cur_chunks.iter();

            if let Some((prev_tick, snap)) = it
                .next()
                .and_then(|(&prev_tick, chunk)| chunk.snapshot.as_ref().map(|s| (prev_tick, s)))
            {
                let mut snapshot = MtPoolCow::new_without_pool();
                snapshot.to_mut().extend(snap.iter());
                let local_players = game.build_from_snapshot(&snapshot);
                let next_tick = if let Some((next_tick, snap)) = it
                    .next()
                    .and_then(|(&next_tick, chunk)| chunk.snapshot.as_ref().map(|s| (next_tick, s)))
                {
                    let mut snapshot = MtPoolCow::new_without_pool();
                    snapshot.to_mut().extend(snap.iter());
                    game.build_from_snapshot_for_pred(&snapshot);
                    next_tick
                } else {
                    prev_tick
                };
                Some((local_players, prev_tick, next_tick))
            } else {
                None
            }
        } else {
            None
        }) else {
            return Err(anyhow!("no snapshots inside this demo"));
        };

        let (player_id, _) = local_players.iter().next().unwrap();
        let intra_tick_time = self.inner.intra_tick_time();

        let render_for_player = RenderForPlayer {
            chat_info: None,
            emote_wheel_info: None,
            scoreboard_active: false,
            chat_show_all: false,

            local_player_info: game.collect_character_local_render_info(player_id),

            player_id: *player_id,
        };

        let game_time_info = GameTimeInfo {
            ticks_per_second: game.game_tick_speed(),
            intra_tick_time,
        };
        let intra_tick_ratio = self.inner.intra_tick_ratio(prev_tick, next_tick);

        let character_infos = game.collect_characters_info();

        let stages = game.all_stages(intra_tick_ratio);

        let scoreboard_info = None; // game.collect_scoreboard_info();

        let chat_msgs = PoolVecDeque::new_without_pool();
        let mut render_game_input = RenderGameInput {
            players: PoolVec::new_without_pool(),
            events: None,
            vote: None,
            character_infos,
            stages,
            scoreboard_info,
            chat_msgs,
            game_time_info,
            settings: Default::default(),
        };

        let render_for_player = RenderGameForPlayer {
            render_for_player,
            observed_players: PoolVec::new_without_pool(),
            observed_anchored_size_props: Default::default(),
        };

        render_game_input.players.push(render_for_player);

        /*
            TODO: this is for video encoding
            self.canvas_handle
                .switch_canvas(CommandSwitchCanvasModeType::Offscreen {
                    id: 0,
                    width: 800,
                    height: 600,
                    has_multi_sampling: None,
                });
        */

        render.render(&self.config_map, &cur_time, render_game_input);
        self.canvas_handle
            .switch_canvas(CommandSwitchCanvasModeType::Onscreen);

        Ok(())
    }
}

impl Drop for DemoViewerImpl {
    fn drop(&mut self) {
        if let Some((encoder, file)) = self.av_encoder.take() {
            drop(encoder);
            let file = Arc::into_inner(file).unwrap().into_inner().unwrap();
            let fs = self.io.fs.clone();
            self.io.io_batcher.spawn_without_lifetime(async move {
                fs.write_file("testtest.webm".as_ref(), file).await?;
                Ok(())
            });
        }
    }
}

pub struct DemoViewerLoading {
    pub task: IoBatcherTask<DemoContainer>,
    pub io: Io,
    pub thread_pool: Arc<rayon::ThreadPool>,
    pub fonts: Arc<UiFontData>,
}

pub struct DemoViewerLoadingComponents {
    pub client_map: ClientMapLoading,
    pub demo: DemoContainer,
    pub io: Io,
    pub fonts: Arc<UiFontData>,
}

pub enum DemoViewer {
    Loading(Box<DemoViewerLoading>),
    LoadingComponents(Box<DemoViewerLoadingComponents>),
    Rendering(Box<DemoViewerImpl>),
    None,
}

impl DemoViewer {
    pub fn new(
        io: &Io,
        thread_pool: &Arc<rayon::ThreadPool>,
        demo_name: String,
        fonts: Arc<UiFontData>,
    ) -> Self {
        let fs = io.fs.clone();
        let demo_name = demo_name.to_string();
        let read_demo = io.io_batcher.spawn(async move {
            let demo = fs
                .read_file(format!("demos/{}", demo_name).as_ref())
                .await?;

            fn decomp<'a>(v: &[u8], writer: &'a mut Vec<u8>) -> anyhow::Result<&'a [u8]> {
                writer.clear();
                let mut decoder = zstd::Decoder::new(v)?;
                decoder.read_to_end(&mut *writer)?;
                decoder.finish();

                Ok(writer.as_mut_slice())
            }
            fn deser_ex<T: DeserializeOwned>(
                v: &[u8],
                fixed_size: bool,
            ) -> anyhow::Result<(T, usize)> {
                if fixed_size {
                    Ok(bincode::serde::decode_from_slice(
                        v,
                        bincode::config::standard().with_fixed_int_encoding(),
                    )?)
                } else {
                    Ok(bincode::serde::decode_from_slice(
                        v,
                        bincode::config::standard(),
                    )?)
                }
            }
            fn deser<T: DeserializeOwned>(v: &[u8]) -> anyhow::Result<(T, usize)> {
                deser_ex(v, false)
            }

            let mut writer: Vec<u8> = Default::default();

            // read header
            let (header, file_off): (DemoHeader, usize) = deser_ex(&demo, true)?;
            let demo = &demo[file_off..];

            // read header ext
            let (header_ext, _): (DemoHeaderExt, usize) =
                deser(decomp(&demo[0..header.size_ext as usize], &mut writer)?)?;

            let demo = &demo[header.size_ext as usize..];
            let chunks = &demo[0..header.size_chunks as usize];
            let tail = &demo[header.size_chunks as usize..];

            // read tail
            let (tail, _): (DemoTail, usize) = deser(decomp(tail, &mut writer)?)?;
            anyhow::ensure!(!tail.index.is_empty(), "no index found in demo tail.");

            // read all chunks
            Ok(DemoContainer {
                header,
                header_ext,
                demo_chunks: chunks.to_vec(),
                tail,
            })
        });
        Self::Loading(Box::new(DemoViewerLoading {
            task: read_demo,
            io: io.clone(),
            thread_pool: thread_pool.clone(),
            fonts,
        }))
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
            DemoViewer::Loading(loading) => {
                let DemoViewerLoading {
                    task,
                    io,
                    thread_pool,
                    fonts,
                } = *loading;
                if task.is_finished() {
                    let demo_container = task.get_storage()?;
                    *self = Self::LoadingComponents(Box::new(DemoViewerLoadingComponents {
                        client_map: ClientMapLoading::new(
                            "map/maps/".as_ref(),
                            &demo_container.header_ext.map,
                            Some(demo_container.header_ext.map_hash),
                            None,
                            &io,
                            &thread_pool,
                            demo_container.header_ext.physics_mod.clone(),
                            false,
                            demo_container.header_ext.game_options.clone(),
                        ),
                        demo: demo_container,
                        io,
                        fonts,
                    }));
                } else {
                    *self = Self::Loading(Box::new(DemoViewerLoading {
                        task,
                        io,
                        thread_pool,
                        fonts,
                    }));
                }
            }
            DemoViewer::LoadingComponents(loading) => {
                let DemoViewerLoadingComponents {
                    mut client_map,
                    demo,
                    io,
                    fonts,
                } = *loading;
                if client_map
                    .continue_loading(sound, graphics, backend, config, sys, &fonts)
                    .is_some()
                {
                    // finished loading client
                    *self = Self::Rendering(Box::new(DemoViewerImpl::new(
                        client_map,
                        &graphics.canvas_handle,
                        backend,
                        demo,
                        sys,
                        io,
                    )));
                } else {
                    *self = Self::LoadingComponents(Box::new(DemoViewerLoadingComponents {
                        client_map,
                        demo,
                        io,
                        fonts,
                    }));
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
