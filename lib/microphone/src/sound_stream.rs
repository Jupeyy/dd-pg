use std::{
    collections::VecDeque,
    fmt::Debug,
    num::NonZeroUsize,
    sync::{Arc, Mutex},
};

use anyhow::anyhow;
use base::join_thread::JoinThread;
use crossbeam::channel::Receiver;
use df::tract::DfParams;
use hiarc::Hiarc;
use rubato::Resampler;
use sound::stream::{DecodeError, StreamDecoder, StreamFrame};

use crate::{
    noise_gate::NoiseGateAndBooster, stream_sample::StreamSample, MicrophoneStream,
    MicrophoneStreamInner, NoiseFilterSettings, NoiseGateSettings,
};

pub const OPUS_SAMPLE_RATE: usize = 48000;
pub const OPUS_10_MS: usize = OPUS_SAMPLE_RATE * 10 / 1000;

#[atomic_enum::atomic_enum]
pub enum SoundStreamSpeed {
    None,
    TooSlow,
    TooFast,
}

pub struct SoundStreamResampled {
    fast_resampler: rubato::FastFixedIn<f32>,
    slow_resampler: rubato::FastFixedIn<f32>,

    sample_buffer: Vec<f32>,
    resample_buffer: Vec<f32>,

    pending_frames: VecDeque<Vec<StreamFrame>>,
}

impl Debug for SoundStreamResampled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoundStreamResampled")
            .field("pending_frames", &self.pending_frames)
            .finish()
    }
}

impl Default for SoundStreamResampled {
    fn default() -> Self {
        let fast_resampler = rubato::FastFixedIn::<f32>::new(
            0.9,
            2.0,
            rubato::PolynomialDegree::Linear,
            OPUS_10_MS,
            1,
        )
        .unwrap();

        let slow_resampler = rubato::FastFixedIn::<f32>::new(
            1.1,
            2.0,
            rubato::PolynomialDegree::Linear,
            OPUS_10_MS,
            1,
        )
        .unwrap();

        Self {
            fast_resampler,
            slow_resampler,
            sample_buffer: Default::default(),
            resample_buffer: Default::default(),
            pending_frames: Default::default(),
        }
    }
}

impl SoundStreamResampled {
    pub fn resample_and_push(&mut self, samples: Vec<StreamFrame>, faster: bool) {
        self.sample_buffer.clear();
        self.sample_buffer
            .extend(samples.into_iter().map(|f| f.left));

        let resampler = if faster {
            &mut self.fast_resampler
        } else {
            &mut self.slow_resampler
        };

        resampler.reset();

        self.resample_buffer
            .resize(resampler.output_frames_max(), 0.0);
        let (_, count) = resampler
            .process_into_buffer(
                &[&self.sample_buffer],
                &mut [&mut self.resample_buffer],
                None,
            )
            .unwrap();

        self.pending_frames.push_back(
            self.resample_buffer[0..count]
                .iter()
                .map(|&f| StreamFrame { left: f, right: f })
                .collect(),
        );
    }
}

#[derive(Debug, Hiarc)]
pub struct SoundStreamInner {
    receiver: Receiver<Vec<StreamFrame>>,
    slow_stream_open_threshold: NonZeroUsize,
    good_stream_threshold: NonZeroUsize,

    #[hiarc_skip_unsafe]
    speed: AtomicSoundStreamSpeed,

    #[hiarc_skip_unsafe]
    resampled: Mutex<SoundStreamResampled>,
}

impl SoundStreamInner {
    fn determine_speed(&self) {
        let len = self.receiver.len();
        if len > self.slow_stream_open_threshold.get() {
            self.speed.store(
                SoundStreamSpeed::TooSlow,
                std::sync::atomic::Ordering::Relaxed,
            );
        } else if len >= 1 && len <= self.good_stream_threshold.get() {
            self.speed
                .store(SoundStreamSpeed::None, std::sync::atomic::Ordering::Relaxed);
        }
    }

    fn resample(&self) {
        let speed = self.speed.load(std::sync::atomic::Ordering::Relaxed);

        match speed {
            SoundStreamSpeed::None => {
                // Nothing to do
            }
            SoundStreamSpeed::TooSlow => {
                // Make the next buffer run faster
                if let Ok(pending_frame) = self.receiver.try_recv() {
                    let resampled = &mut *self.resampled.lock().unwrap();
                    resampled.resample_and_push(pending_frame, true);
                }
            }
            SoundStreamSpeed::TooFast => {
                // Make the next buffer run slower
                if let Ok(pending_frame) = self.receiver.try_recv() {
                    let resampled = &mut *self.resampled.lock().unwrap();
                    resampled.resample_and_push(pending_frame, false);
                }
            }
        }
    }
}

#[derive(Debug, Hiarc, Clone)]
pub struct SoundStreamImpl(Arc<SoundStreamInner>);

#[derive(Debug, Default, Copy, Clone)]
pub struct SoundStreamettings {
    /// Has to be `Some` to use a noise filter at all
    pub nf: Option<NoiseFilterSettings>,
    pub noise_gate: Option<NoiseGateSettings>,
    /// Boost in db
    pub boost: f64,
}

pub struct SoundStream {
    inner: SoundStreamImpl,
    // keep this for sound streams that have a microphone, else the cpal stream is dropped
    _microphone_inner: Option<MicrophoneStreamInner>,
    // thread last
    _decoder_thread: JoinThread<()>,
}

impl SoundStream {
    pub fn from_receiver(
        opus_receiver: Receiver<StreamSample>,
        inner: Option<MicrophoneStreamInner>,
        settings: SoundStreamettings,
    ) -> Self {
        let (sender, receiver) = crossbeam::channel::bounded(4096);

        let decoder_thread = std::thread::spawn(move || {
            let mut buffer: Vec<i16> = Default::default();
            let mut buffer_float: Vec<f32> = Default::default();
            let mut buffer_float_helper: Vec<f32> = Default::default();
            let mut decoder =
                opus::Decoder::new(OPUS_SAMPLE_RATE as u32, opus::Channels::Mono).unwrap();

            let mut noise_gate = NoiseGateAndBooster::new(
                settings
                    .noise_gate
                    .map(|n| n.open_threshold as f32)
                    .unwrap_or(-200.0),
                settings
                    .noise_gate
                    .map(|n| n.close_threshold as f32)
                    .unwrap_or(-200.0),
                48000.0,
                1,
                150.0,
                25.0,
                150.0,
                settings.boost as f32,
            );

            let mut df = if let Some(nf) = settings.nf {
                let df = DfParams::default();
                let mut rp = df::tract::RuntimeParams::default_with_ch(1)
                    .with_post_filter(0.02)
                    .with_atten_lim(nf.attenuation as f32);
                rp.min_db_thresh = nf.processing_threshold as f32;
                Some(df::tract::DfTract::new(df, &rp).unwrap())
            } else {
                None
            };

            while let Ok(data) = opus_receiver.recv() {
                buffer.resize(OPUS_10_MS, 0);
                let count = decoder
                    .decode(&data.data, buffer.as_mut_slice(), false)
                    .unwrap();
                buffer_float.clear();
                buffer_float.extend(
                    buffer
                        .drain(0..count)
                        .map(|sample| (sample as f64 / i16::MAX as f64) as f32),
                );

                if settings.boost != 0.0 || settings.noise_gate.is_some() {
                    buffer_float_helper.resize(count, 0.0);
                    noise_gate.process_frame(&buffer_float, &mut buffer_float_helper);
                    std::mem::swap(&mut buffer_float, &mut buffer_float_helper);
                }

                if let Some(df) = &mut df {
                    buffer_float_helper.resize(count, 0.0);

                    let nf_in =
                        ndarray::ArrayView2::from_shape((1, buffer_float.len()), &buffer_float)
                            .unwrap();
                    let nf_out = ndarray::ArrayViewMut2::from_shape(
                        (1, buffer_float_helper.len()),
                        &mut buffer_float_helper,
                    )
                    .unwrap();

                    if let Err(err) = df.process(nf_in, nf_out) {
                        log::info!("Err from noise filter: {err}");
                    }
                    std::mem::swap(&mut buffer_float, &mut buffer_float_helper);
                }

                sender
                    .send(
                        buffer_float
                            .drain(..)
                            .map(|sample| StreamFrame {
                                left: sample,
                                right: sample,
                            })
                            .collect(),
                    )
                    .unwrap();
            }
        });

        Self {
            inner: SoundStreamImpl(Arc::new(SoundStreamInner {
                receiver,
                slow_stream_open_threshold: 12.try_into().unwrap(),
                good_stream_threshold: 4.try_into().unwrap(),

                speed: AtomicSoundStreamSpeed::new(SoundStreamSpeed::None),

                resampled: Default::default(),
            })),
            _microphone_inner: inner,
            _decoder_thread: JoinThread::new(decoder_thread),
        }
    }

    pub fn new(microphone: MicrophoneStream, settings: SoundStreamettings) -> Self {
        let (inner, opus_receiver) = microphone.split();
        Self::from_receiver(opus_receiver, Some(inner), settings)
    }

    pub fn stream(&self) -> Arc<SoundStreamImpl> {
        Arc::new(self.inner.clone())
    }
}

impl StreamDecoder for SoundStreamImpl {
    fn sample_rate(&self) -> u32 {
        OPUS_SAMPLE_RATE as u32
    }

    fn num_frames(&self) -> usize {
        usize::MAX
    }

    fn decode(&self) -> Result<Vec<sound::stream::StreamFrame>, DecodeError> {
        let inner = &self.0;
        // this stream is too slow, drop older packets
        inner.determine_speed();
        inner.resample();

        if let Some(pending) = inner.resampled.lock().unwrap().pending_frames.pop_front() {
            Ok(pending)
        } else {
            match inner.receiver.try_recv().map_err(|err| match err {
                crossbeam::channel::TryRecvError::Empty => false,
                crossbeam::channel::TryRecvError::Disconnected => true,
            }) {
                Ok(res) => Ok(res),
                // fill with empty values, since stream needs to recover
                Err(false) => {
                    inner.speed.store(
                        SoundStreamSpeed::TooFast,
                        std::sync::atomic::Ordering::Relaxed,
                    );
                    Err(DecodeError::MustGenerateEmpty(OPUS_10_MS))
                }
                _ => Err(DecodeError::Err(anyhow!("Channel closed."))),
            }
        }
    }

    fn seek(&self, index: usize) -> Result<usize, anyhow::Error> {
        Ok(index)
    }
}
