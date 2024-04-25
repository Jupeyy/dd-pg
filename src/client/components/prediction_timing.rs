use std::{collections::VecDeque, time::Duration};

struct PredictionTimingCollection {
    max: Duration,
    min: Duration,
    average: Duration,
    avg_count: usize,
}

pub struct PredictionTiming {
    /// last 10 seconds of highest/lowest/average pings per second
    last_10_secs_of_pings: VecDeque<PredictionTimingCollection>,
    /// last 10 snap differences in timing
    last_10_secs_of_snaps: VecDeque<(f64, usize)>,
    /// max of the last 10 seconds of frame times
    frame_time_max: VecDeque<Duration>,

    cur_whole_second: u64,
    cur_whole_second_snap: u64,
    cur_whole_second_frametime: u64,
}

impl PredictionTiming {
    /// small "extra" offset to smooth out some _normal_ jitter in frame times
    pub const PREDICTION_MARGIN: Duration = Duration::from_millis(1);

    pub fn new(first_ping: Duration, cur_time: Duration) -> Self {
        Self {
            last_10_secs_of_pings: vec![PredictionTimingCollection {
                max: first_ping,
                min: first_ping,
                average: first_ping,
                avg_count: 1,
            }]
            .into(),
            last_10_secs_of_snaps: vec![(0.0, 1)].into(),
            frame_time_max: vec![Duration::ZERO].into(),
            cur_whole_second: cur_time.as_secs(),
            cur_whole_second_snap: cur_time.as_secs(),
            cur_whole_second_frametime: cur_time.as_secs(),
        }
    }

    pub fn add(&mut self, ping: Duration, cur_time: Duration) {
        let whole_second = cur_time.as_secs().max(self.cur_whole_second);

        if whole_second > self.cur_whole_second {
            let diff = whole_second - self.cur_whole_second;
            self.last_10_secs_of_pings
                .truncate((10 - diff.min(10)) as usize);
            self.last_10_secs_of_pings
                .push_front(PredictionTimingCollection {
                    max: ping,
                    min: ping,
                    average: ping,
                    avg_count: 1,
                });
        } else {
            let cur_average = &mut self.last_10_secs_of_pings[0];
            cur_average.max = cur_average.max.max(ping);
            cur_average.min = cur_average.min.min(ping);
            cur_average.avg_count += 1;
            cur_average.average =
                Duration::from_nanos((cur_average.average.as_nanos() + ping.as_nanos()) as u64);
        }
        self.cur_whole_second = whole_second;
    }

    /// get's the highest value
    pub fn max(&self) -> Duration {
        self.last_10_secs_of_pings
            .iter()
            .max_by(|a1, a2| a1.max.cmp(&a2.max))
            .map(|v| v.max)
            .unwrap()
    }

    /// get's the lowest value
    pub fn min(&self) -> Duration {
        self.last_10_secs_of_pings
            .iter()
            .min_by(|a1, a2| a1.min.cmp(&a2.min))
            .map(|v| v.min)
            .unwrap()
    }

    /// get's the average value
    pub fn average(&self) -> Duration {
        let count = self.last_10_secs_of_pings.len().max(1);
        Duration::from_nanos(
            (self
                .last_10_secs_of_pings
                .iter()
                .map(|ping| ping.average.as_nanos() / ping.avg_count as u128)
                .sum::<u128>()
                / count as u128) as u64,
        )
    }

    pub fn add_snap(&mut self, snap_diff: f64, timestamp: Duration) {
        let whole_second = timestamp.as_secs().max(self.cur_whole_second_snap);

        if whole_second > self.cur_whole_second_snap {
            let diff = whole_second - self.cur_whole_second_snap;
            self.last_10_secs_of_snaps
                .truncate((10 - diff.min(10)) as usize);
            self.last_10_secs_of_snaps.push_front((snap_diff, 1));
        } else {
            let (cur_average, avg_count) = &mut self.last_10_secs_of_snaps[0];
            *avg_count += 1;
            *cur_average = *cur_average + snap_diff;
        }
        self.cur_whole_second_snap = whole_second;
    }

    /// get's the average snap diff value
    pub fn average_snap(&self) -> f64 {
        let count = self.last_10_secs_of_snaps.len().max(1);

        // by default only look at the last 20 snapshots
        let count = 20.min(count);
        let weights = (0..count).map(|index| (count - index) as f64 / count as f64);
        self.last_10_secs_of_snaps
            .iter()
            .take(count)
            .zip(weights.clone())
            .map(|(&(val, avg_count), weight)| (val / avg_count as f64) * weight)
            .sum::<f64>()
            / weights.clone().sum::<f64>()
    }

    pub fn add_frametime(&mut self, time: Duration, cur_time: Duration) {
        let whole_second = cur_time.as_secs().max(self.cur_whole_second_frametime);

        if whole_second > self.cur_whole_second_frametime {
            let diff = whole_second - self.cur_whole_second_frametime;
            self.frame_time_max.truncate((2 - diff.min(2)) as usize);
            self.frame_time_max.push_front(time);
        } else {
            let cur_average = &mut self.frame_time_max[0];
            *cur_average = (*cur_average).max(time);
        }
        self.cur_whole_second_frametime = whole_second;
    }

    pub fn max_frametime(&self) -> Duration {
        *self.frame_time_max.iter().max().unwrap() + Self::PREDICTION_MARGIN
    }
}
