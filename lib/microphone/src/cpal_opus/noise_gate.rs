//! From https://github.com/charlesportwoodii/noise-gate/blob/7638b1a5af7e1fa8abda04884f5c32656d432372/src/lib.rs
//!
//! Modified for our needs.
//! BSD-3-Clause
//! Copyright 2024 - Present Charles R. Portwood II <charlesportwoodii@erianna.com>

fn db_to_ratio(v: f32) -> f32 {
    (10_f32).powf(v / 20.0)
}
fn ratio_to_db(v: f32) -> f32 {
    v.log10() * 20.0
}

/// The Noise Gate & Booster
/// This should be implemented outside of your main audio loop so that the open/close thresholds, and other settings can persist across the stream
pub struct NoiseGateAndBooster {
    /// The open threshold as db (eg -36.0)
    open_threshold: f32,
    /// The close threshold as db (eg -56.0)
    close_threshold: f32,
    /// The sample rate in hz (eg 48000.0)
    sample_rate: f32,
    /// The relesae rate, in ms (eg 150)
    release_rate: f32,
    /// The attack rate in ms
    attack_rate: f32,
    decay_rate: f32,
    /// How long the gate should be held open for in ms
    hold_time: f32,
    /// The number of audio channels in your stream
    channels: usize,
    is_open: bool,
    attenuation: f32,
    level: f32,
    held_time: f32,
    boost: f32,

    channel_frames: Vec<Vec<f32>>,
}

impl NoiseGateAndBooster {
    /// Create a new noise gate.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        open_threshold: f32,
        close_threshold: f32,
        sample_rate: f32,
        channels: usize,
        release_rate: f32,
        attack_rate: f32,
        hold_time: f32,
        boost: f32,
    ) -> Self {
        let threshold_diff = open_threshold - close_threshold;
        let min_decay_period = (1.0 / 75.0) * sample_rate;

        Self {
            open_threshold: match open_threshold.is_finite() {
                true => db_to_ratio(open_threshold),
                false => 0.0,
            },
            close_threshold: match close_threshold.is_finite() {
                true => db_to_ratio(close_threshold),
                false => 0.0,
            },
            sample_rate: 1.0 / sample_rate,
            channels,
            release_rate: 1.0 / (release_rate * 0.001 * sample_rate),
            attack_rate: 1.0 / (attack_rate * 0.001 * sample_rate),
            decay_rate: threshold_diff / min_decay_period,
            hold_time: hold_time * 0.001,
            is_open: false,
            attenuation: 0.0,
            level: 0.0,
            held_time: 0.0,
            channel_frames: Default::default(),
            boost,
        }
    }

    /// Takes a frame and returns a new frame that has been attenuated by the gate
    pub fn process_frame(&mut self, frame: &[f32], output: &mut [f32]) {
        let channel_frames = &mut self.channel_frames;
        channel_frames.resize_with(self.channels, || {
            Vec::<f32>::with_capacity(frame.len() / self.channels)
        });
        channel_frames.truncate(self.channels);

        for (c, channel_frame) in channel_frames.iter_mut().enumerate() {
            channel_frame.clear();
            if self.boost != 0.0 {
                let peak_ratio = frame
                    .iter()
                    .skip(c)
                    .step_by(self.channels)
                    .max_by(|s1, s2| s1.abs().total_cmp(&s2.abs()))
                    .map(|s| s.abs())
                    .unwrap();
                let peek_ratio_boosted = db_to_ratio({
                    let db = ratio_to_db(peak_ratio);
                    db + peak_ratio.signum() * self.boost
                });
                let boost_factor = if peak_ratio > 0.01 {
                    peek_ratio_boosted / peak_ratio
                } else {
                    1.0
                };
                for u in frame.iter().skip(c).step_by(self.channels) {
                    channel_frame.push(*u * boost_factor);
                }
            } else {
                for u in frame.iter().skip(c).step_by(self.channels) {
                    channel_frame.push(*u);
                }
            }
        }

        for i in 0..channel_frames[0].len() {
            let mut current_level = f32::abs(channel_frames[0][i]);

            for channel_frame in channel_frames.iter_mut() {
                current_level = f32::max(current_level, channel_frame[i]);
            }

            if current_level > self.open_threshold && !self.is_open {
                self.is_open = true;
            }

            if self.level < self.close_threshold && self.is_open {
                self.held_time = 0.0;
                self.is_open = false;
            }

            self.level = f32::max(self.level, current_level) - self.decay_rate;

            if self.is_open {
                self.attenuation = f32::min(1.0, self.attenuation + self.attack_rate);
            } else {
                self.held_time += self.sample_rate;
                if self.held_time > self.hold_time {
                    self.attenuation = f32::max(0.0, self.attenuation - self.release_rate);
                }
            }

            for channel_frame in channel_frames.iter_mut() {
                channel_frame[i] *= self.attenuation;
            }
        }

        // We need to flatten this back down to a single vec
        // For each channel
        // Grab the next element and push it to resample
        for i in 0..channel_frames[0].len() {
            for c in 0..self.channels {
                output[i * self.channels + c] = channel_frames[c][i];
            }
        }
    }
}
