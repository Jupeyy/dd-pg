use std::{
    cell::Cell,
    collections::{BTreeMap, HashMap},
    fs::File,
    io::{Seek, Write},
    path::Path,
    sync::mpsc::{Receiver, Sender},
    thread::JoinHandle,
    time::Duration,
};

use anyhow::anyhow;
use base::{hash::Hash, reduced_ascii_str::ReducedAsciiString};
use base_io::io::Io;
use game_interface::{
    interface::GameStateCreateOptions,
    types::{game::NonZeroGameTickType, network_string::NetworkReducedAsciiString},
};
use itertools::Itertools;
use serde::Serialize;

use crate::{
    ChunkHeader, DemoEvent, DemoEvents, DemoGameModification, DemoHeader, DemoHeaderExt,
    DemoSnapshot, DemoTail, DATA_PER_CHUNK_TO_WRITE, SECONDS_UNTIL_WRITE,
};

#[derive(Debug, Clone)]
pub struct DemoRecorderCreateProps {
    pub map: ReducedAsciiString,
    pub map_hash: Hash,
    pub game_options: GameStateCreateOptions,
    pub required_resources: HashMap<String, String>,
    pub physics_module: DemoGameModification,
    pub render_module: DemoGameModification,
    pub physics_group_name: NetworkReducedAsciiString<24>,
    pub io: Io,
}

enum DemoRecorderChunk {
    Snapshots { snaps: BTreeMap<u64, DemoSnapshot> },
    Events { events: BTreeMap<u64, DemoEvents> },
}

/// Records demos from snapshots & events
pub struct DemoRecorder {
    /// The dynamic sized header is only written once
    pub demo_header_ext: DemoHeaderExt,
    /// current demo snapshots
    pub snapshots: BTreeMap<u64, DemoSnapshot>,
    pub events: BTreeMap<u64, DemoEvents>,

    /// Event sender for the writer thread.
    /// Must stay to not be dropped
    thread_sender: Sender<DemoRecorderChunk>,
    /// the thread that writes all demo changes to disk
    _writer_thread: JoinHandle<()>,
}

impl DemoRecorder {
    pub fn new(
        props: DemoRecorderCreateProps,
        ticks_per_second: NonZeroGameTickType,
        mut forced_name: Option<String>,
    ) -> Self {
        let (thread_sender, recv) = std::sync::mpsc::channel();

        let now = chrono::Utc::now();
        let demo_name = forced_name
            .take()
            .unwrap_or_else(|| format!("{}_{}", props.map.as_str(), now.format("%Y_%m_%d_%H_%M")));

        let demo_header_ext = DemoHeaderExt {
            server: "".into(),
            physics_mod: props.physics_module,
            render_mod: props.render_module,
            required_resources: props.required_resources,
            map: props.map,
            map_hash: props.map_hash,
            ticks_per_second,
            game_options: props.game_options,
            physics_group_name: props.physics_group_name,
        };

        let io = props.io;

        let tmp_demo_dir = io.fs.get_save_path().join("tmp/demos");
        let demo_dir = io.fs.get_save_path().join("demos");
        let demo_header_ext_thread = demo_header_ext.clone();
        let writer_thread = std::thread::Builder::new()
            .name(format!("demo-recorder-{}", demo_header_ext.map.as_str()))
            .spawn(move || {
                Self::writer_thread_run(
                    &tmp_demo_dir,
                    &demo_dir,
                    &demo_name,
                    recv,
                    demo_header_ext_thread,
                )
                .unwrap()
            })
            .expect("could not spawn a demo-recorder thread.");

        Self {
            demo_header_ext,
            snapshots: Default::default(),
            events: Default::default(),

            thread_sender,
            _writer_thread: writer_thread,
        }
    }

    fn writer_thread_run(
        tmp_path: &Path,
        final_path: &Path,
        demo_name: &str,
        recv: Receiver<DemoRecorderChunk>,
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

        fn comp<'a>(
            v: &[u8],
            writer: &'a mut Vec<u8>,
            clear_writer: bool,
        ) -> anyhow::Result<&'a mut [u8]> {
            if clear_writer {
                writer.clear();
            }
            let mut encoder = zstd::Encoder::new(&mut *writer, 0)?;
            encoder.write_all(v)?;
            encoder.finish()?;
            Ok(writer.as_mut_slice())
        }

        fn write(size: &Cell<usize>, file: &mut File, v: &[u8]) -> anyhow::Result<()> {
            size.set(size.get() + v.len());
            Ok(file.write_all(v)?)
        }

        let mut write_ser = Vec::new();
        let mut write_comp = Vec::new();
        let mut write_dst = Vec::new();
        let mut write_data = Vec::new();

        let header_ext_file = comp(ser(&header_ext, &mut write_ser)?, &mut write_comp, true)?;
        let header_ext_len = header_ext_file.len();

        write(
            &size,
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

        write(&size, &mut *file, header_ext_file)?;

        let mut first_monotonic_snaps = None;
        let mut last_monotonic_snaps = None;
        let mut first_monotonic_events = None;
        let mut last_monotonic_events = None;

        let mut events_index: BTreeMap<u64, u64> = Default::default();
        let mut snapshots_index: BTreeMap<u64, u64> = Default::default();

        let size_before_chunks = size.get();

        fn write_chunk<'a, A: Serialize>(
            chunk: BTreeMap<u64, A>,
            writer: &'a mut Vec<u8>,
            tmp: &mut Vec<u8>,
            tmp_dst: &mut Vec<u8>,
            tmp_patch_data: &mut Vec<u8>,
        ) -> anyhow::Result<&'a [u8]> {
            writer.clear();

            let mut last_data: Option<Vec<u8>> = None;

            // first write chunk count
            let len_ser = ser(&(chunk.len() as u64), &mut *tmp)?;
            writer.write_all(len_ser)?;

            for (monotonic_tick, data) in chunk {
                tmp_patch_data.clear();

                // prepare optimized data
                let data = {
                    let data_serialized = ser(&data, tmp_dst)?;
                    let data = if let Some(last_data) = &last_data {
                        bin_patch::diff(last_data, data_serialized, &mut *tmp_patch_data)?;
                        Some(tmp_patch_data.as_mut_slice())
                    } else {
                        Some(comp(data_serialized, tmp_patch_data, true)?)
                    };
                    last_data = Some(data_serialized.to_vec());
                    data
                };

                let mono_ser = ser(
                    &ChunkHeader {
                        monotonic_tick,
                        size: data.as_ref().map(|s| s.len() as u64).unwrap_or_default(),
                    },
                    &mut *tmp,
                )?;
                writer.write_all(mono_ser)?;
                // now write the data
                if let Some(data) = data {
                    writer.write_all(data)?;
                }
            }

            tmp_dst.clear();
            tmp_dst.extend(0_u64.to_le_bytes());
            comp(writer, tmp_dst, false)?;
            // write size
            let size = (tmp_dst.len() - std::mem::size_of::<u64>()) as u64;
            tmp_dst[0..std::mem::size_of::<u64>()].copy_from_slice(&size.to_le_bytes());
            std::mem::swap(writer, tmp_dst);
            Ok(writer.as_mut_slice())
        }

        #[allow(clippy::too_many_arguments)]
        fn serialize_and_write_chunk<A: Serialize>(
            file: &mut File,
            index: &mut BTreeMap<u64, u64>,
            chunk: BTreeMap<u64, A>,
            size: &Cell<usize>,
            size_before_chunks: usize,
            first_monotonic: &mut Option<u64>,
            last_monotonic: &mut Option<u64>,

            write_ser: &mut Vec<u8>,
            write_comp: &mut Vec<u8>,
            write_dst: &mut Vec<u8>,
            write_data: &mut Vec<u8>,
        ) -> anyhow::Result<()> {
            let first_tick = chunk
                .first_key_value()
                .map(|(c, _)| *c)
                .ok_or_else(|| anyhow!("empty chunks are not allowed."))?;

            let last_tick = chunk
                .last_key_value()
                .map(|(c, _)| *c)
                .ok_or_else(|| anyhow!("empty chunks are not allowed."))?;

            index.insert(first_tick, (size.get() - size_before_chunks) as u64);

            write(
                size,
                &mut *file,
                write_chunk(chunk, write_ser, write_comp, write_dst, write_data)?,
            )?;

            let monotonic_first_tick = *first_monotonic.get_or_insert(first_tick);
            anyhow::ensure!(
                monotonic_first_tick <= first_tick,
                "somehow the first recorded monotonic tick was bigger than the current first tick (so not monotonic)."
            );
            anyhow::ensure!(
                last_tick >= last_monotonic.unwrap_or_default(),
                "somehow the current last monotonic tick was smaller than the last one recorded (so not monotonic)."
            );
            let monotonic_last_tick = *last_monotonic.insert(last_tick);
            anyhow::ensure!(
                monotonic_last_tick >= monotonic_first_tick,
                "somehow the first monotonic tick was bigger than the current last one."
            );
            Ok(())
        }

        while let Ok(event) = recv.recv() {
            match event {
                DemoRecorderChunk::Snapshots { snaps } => {
                    serialize_and_write_chunk(
                        file,
                        &mut snapshots_index,
                        snaps,
                        &size,
                        size_before_chunks,
                        &mut first_monotonic_snaps,
                        &mut last_monotonic_snaps,
                        &mut write_ser,
                        &mut write_comp,
                        &mut write_dst,
                        &mut write_data,
                    )?;
                }
                DemoRecorderChunk::Events { events } => {
                    serialize_and_write_chunk(
                        file,
                        &mut events_index,
                        events,
                        &size,
                        size_before_chunks,
                        &mut first_monotonic_events,
                        &mut last_monotonic_events,
                        &mut write_ser,
                        &mut write_comp,
                        &mut write_dst,
                        &mut write_data,
                    )?;
                }
            }
        }

        let chunks_size = size.get() - size_before_chunks;

        // `or` to make sure None is never compared if there is one with Some
        // having Some is a must for the next if check
        let first_monotonic = (first_monotonic_snaps.or(first_monotonic_events))
            .min(first_monotonic_events.or(first_monotonic_snaps));
        let last_monotonic = (last_monotonic_snaps.or(last_monotonic_events))
            .max(last_monotonic_events.or(last_monotonic_snaps));

        if let Some((first_monotonic, last_monotonic)) = first_monotonic.zip(last_monotonic) {
            // write the demo tail
            write(
                &size,
                &mut *file,
                comp(
                    ser(
                        &DemoTail {
                            snapshots_index,
                            events_index,
                        },
                        &mut write_ser,
                    )?,
                    &mut write_comp,
                    true,
                )?,
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
            std::fs::rename(path, final_path.join(format!("{}.twdemo", demo_name)))?;
        }
        // else the demo is invalid and can be dropped.

        Ok(())
    }

    fn try_write_chunks<A>(
        data: &mut BTreeMap<u64, Vec<A>>,
        demo_header_ext: &DemoHeaderExt,
        thread_sender: &Sender<DemoRecorderChunk>,
        write: impl FnOnce(BTreeMap<u64, Vec<A>>) -> DemoRecorderChunk,
    ) {
        let get_chunks_th = || {
            let count = data.len();
            // find key as fast as possible
            if count >= (DATA_PER_CHUNK_TO_WRITE * 2) as usize {
                data.keys().nth(DATA_PER_CHUNK_TO_WRITE as usize)
            } else {
                data.keys()
                    .rev()
                    .nth((count - 1) - DATA_PER_CHUNK_TO_WRITE as usize)
            }
        };

        // check if chunks should be flushed
        // and the difference is > 3 seconds
        if data.len() > DATA_PER_CHUNK_TO_WRITE as usize
            && get_chunks_th()
                .copied()
                .zip(data.last_key_value().map(|(&tick, _)| tick))
                .is_some_and(|(first, last)| {
                    last - first > demo_header_ext.ticks_per_second.get() * SECONDS_UNTIL_WRITE
                })
        {
            let key = get_chunks_th();

            if let Some(&key) = key {
                let chunks = data.split_off(&key);
                // bit overcomplicated but split_off :/
                let chunks = std::mem::replace(data, chunks);

                // ignore the error here, if the write thread died, so be it, can't recover anyway.
                let _ = thread_sender.send(write(chunks));
            }
        }
    }

    fn can_add_chunk<A>(
        monotonic_tick: u64,
        data: &mut BTreeMap<u64, Vec<A>>,
        demo_header_ext: &DemoHeaderExt,
    ) -> bool {
        data.is_empty()
            || data.last_key_value().is_some_and(|(&key, _)| {
                monotonic_tick >= key
                    || (key - monotonic_tick)
                        <= demo_header_ext.ticks_per_second.get() * SECONDS_UNTIL_WRITE
            })
    }

    pub fn add_snapshot(&mut self, monotonic_tick: u64, snapshot: Vec<u8>) {
        Self::try_write_chunks(
            &mut self.snapshots,
            &self.demo_header_ext,
            &self.thread_sender,
            |snaps| DemoRecorderChunk::Snapshots { snaps },
        );

        // make sure only snapshots of the last 3 seconds are handled
        if Self::can_add_chunk(monotonic_tick, &mut self.snapshots, &self.demo_header_ext) {
            // if the entry already exist, update if, else create a new
            let entry = self.snapshots.entry(monotonic_tick).or_default();

            *entry = snapshot;
        }
    }

    pub fn add_event(&mut self, monotonic_tick: u64, event: DemoEvent) {
        Self::try_write_chunks(
            &mut self.events,
            &self.demo_header_ext,
            &self.thread_sender,
            |events| DemoRecorderChunk::Events { events },
        );

        // make sure only events of the last 3 seconds are handled
        if Self::can_add_chunk(monotonic_tick, &mut self.events, &self.demo_header_ext) {
            // if the entry already exist, update if, else create a new
            let entry = self.events.entry(monotonic_tick).or_default();

            entry.push(event);
        }
    }
}

impl Drop for DemoRecorder {
    fn drop(&mut self) {
        // write remaining chunks
        fn check_write<A>(
            data: &mut BTreeMap<u64, Vec<A>>,
            thread_sender: &Sender<DemoRecorderChunk>,
            write: impl Fn(BTreeMap<u64, Vec<A>>) -> DemoRecorderChunk,
        ) {
            if !data.is_empty() {
                std::mem::take(data)
                    .into_iter()
                    .chunks(DATA_PER_CHUNK_TO_WRITE as usize)
                    .into_iter()
                    .map(|chunks| chunks.collect::<BTreeMap<_, _>>())
                    .filter(|c| !c.is_empty())
                    .for_each(|chunks| {
                        // ignore the error here, if the write thread died, so be it, can't recover anyway.
                        let _ = thread_sender.send(write(chunks));
                    });
            }
        }
        check_write(&mut self.snapshots, &self.thread_sender, |snaps| {
            DemoRecorderChunk::Snapshots { snaps }
        });
        check_write(&mut self.events, &self.thread_sender, |events| {
            DemoRecorderChunk::Events { events }
        });
    }
}
