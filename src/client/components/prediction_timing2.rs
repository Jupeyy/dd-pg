use std::{collections::VecDeque, time::Duration};

#[derive(Debug)]
struct PredictionTimingCollection {
    max: Duration,
    min: Duration,
    average: Duration,
    avg_count: usize,
}

#[derive(Debug)]
pub struct PredictionTiming {
    /// last seconds of highest/lowest/average pings per second
    last_secs_of_pings: VecDeque<PredictionTimingCollection>,
    smooth_max_ping: f64,
    /// last snap differences in timing
    last_snaps_average: (f64, usize),
    /// max of the last seconds of frame times
    frame_time_max: VecDeque<Duration>,

    cur_whole_second: u64,
    cur_duration_snap: Duration,
    cur_whole_second_frametime: u64,

    /// current smooth time that applied to the timer
    smooth_time: f64,
    jitter_range: Duration,
}

impl PredictionTiming {
    /// small "extra" offset to smooth out some _normal_ jitter in frame times
    pub const PREDICTION_MARGIN: Duration = Duration::from_millis(1);

    pub fn new(first_ping: Duration, cur_time: Duration) -> Self {
        Self {
            last_secs_of_pings: vec![PredictionTimingCollection {
                max: first_ping,
                min: first_ping,
                average: first_ping,
                avg_count: 1,
            }]
            .into(),
            smooth_max_ping: 0.0,
            last_snaps_average: (0.0, 1),
            frame_time_max: vec![Duration::ZERO].into(),
            cur_whole_second: cur_time.as_secs(),
            cur_duration_snap: cur_time,
            cur_whole_second_frametime: cur_time.as_secs(),

            smooth_time: 0.0,
            jitter_range: Duration::ZERO,
        }
    }

    /// the more jitter we got, the more values we use from the past
    /// to have more stability in the values
    fn calc_farsight_of_jitter(&self) -> usize {
        // TODO: random values
        if self.jitter_range.as_millis() < 2 {
            10
        } else if self.jitter_range.as_millis() < 20 {
            50
        } else if self.jitter_range.as_millis() < 100 {
            300
        } else {
            500
        }
    }

    fn calc_snap_farsight_of_jitter(&self) -> Duration {
        // TODO: random values
        if self.jitter_range.as_millis() < 2 {
            Duration::from_millis(1000)
        } else if self.jitter_range.as_millis() < 20 {
            Duration::from_millis(5000)
        } else if self.jitter_range.as_millis() < 100 {
            Duration::from_millis(7000)
        } else {
            Duration::from_millis(20000)
        }
    }

    pub fn add_ping(&mut self, ping: Duration, cur_time: Duration) {
        let whole_second = cur_time.as_secs().max(self.cur_whole_second);

        let old_max = self.ping_max();
        if whole_second > self.cur_whole_second {
            let cur_ping = &self.last_secs_of_pings[0];
            self.jitter_range = cur_ping.max - cur_ping.min;

            let diff = whole_second - self.cur_whole_second;

            let max_items = self.calc_farsight_of_jitter();
            self.last_secs_of_pings
                .truncate((max_items as u64 - diff.min(max_items as u64)) as usize);
            self.last_secs_of_pings
                .push_front(PredictionTimingCollection {
                    max: ping,
                    min: ping,
                    average: ping,
                    avg_count: 1,
                });
        } else {
            let cur_average = &mut self.last_secs_of_pings[0];
            cur_average.max = cur_average.max.max(ping);
            cur_average.min = cur_average.min.min(ping);
            cur_average.avg_count += 1;
            cur_average.average =
                Duration::from_nanos((cur_average.average.as_nanos() + ping.as_nanos()) as u64);
        }
        let max = self.ping_max();
        self.smooth_max_ping =
            (self.smooth_max_ping + old_max.as_secs_f64() - max.as_secs_f64()).clamp(0.0, f64::MAX);
        self.cur_whole_second = whole_second;
    }

    /// get's the highest value
    pub fn ping_max(&self) -> Duration {
        self.last_secs_of_pings
            .iter()
            .take(self.calc_farsight_of_jitter())
            .max_by(|a1, a2| a1.max.cmp(&a2.max))
            .map(|v| v.max)
            .unwrap()
    }

    /// Get's the time on which the average snap time
    /// should balance on. So basically the offset happens
    /// because of ping jitters
    pub fn pred_max_smooth(&mut self) -> Duration {
        let max_ping = self.ping_max();
        let max_frame_time = self.max_frametime();

        // if the jitter is high, except some bigger jumps
        //dbg!(self.smooth_max_ping * 1000.0, max.as_secs_f64() * 1000.0);
        self.smooth_max_ping -= (self.smooth_max_ping * 0.1).clamp(-0.01, 0.01);

        Duration::from_secs_f64(
            (max_ping.as_secs_f64() + max_frame_time.as_secs_f64() + self.smooth_max_ping)
                .clamp(0.0, f64::MAX),
        )
    }

    /// get's the lowest value
    pub fn ping_min(&self) -> Duration {
        self.last_secs_of_pings
            .iter()
            .take(self.calc_farsight_of_jitter())
            .min_by(|a1, a2| a1.min.cmp(&a2.min))
            .map(|v| v.min)
            .unwrap()
    }

    /// get's the average value
    pub fn ping_average(&self) -> Duration {
        let count = self.last_secs_of_pings.len().max(1);
        Duration::from_nanos(
            (self
                .last_secs_of_pings
                .iter()
                .take(self.calc_farsight_of_jitter())
                .map(|ping| ping.average.as_nanos() / ping.avg_count as u128)
                .sum::<u128>()
                / count as u128) as u64,
        )
    }

    pub fn add_frametime(&mut self, time: Duration, cur_time: Duration) {
        let whole_second = cur_time.as_secs().max(self.cur_whole_second_frametime);

        if whole_second > self.cur_whole_second_frametime {
            let diff = whole_second - self.cur_whole_second_frametime;
            let max_seconds = 2;
            self.frame_time_max
                .truncate((max_seconds - diff.min(max_seconds)) as usize);
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

    fn dist_ratio(x: f64, min: f64, max: f64, mid: f64) -> f64 {
        let relative_x = mid + x;
        let dist = if relative_x <= mid {
            let d = mid - min;
            if d < 0.00000001 {
                f64::MAX
            } else {
                (relative_x - mid).abs() / d
            }
        } else {
            let d = max - mid;
            if d < 0.00000001 {
                f64::MAX
            } else {
                (relative_x - mid).abs() / d
            }
        };

        // use expontential function
        let alpha = 10.0;
        let exp_distance = (alpha * dist.abs().clamp(0.0, 1.0)).exp() - 1.0;
        let exp_max = alpha.exp() - 1.0;

        (exp_distance / exp_max).clamp(0.0, 1.0)
    }

    /// if time should be adjusted, returns so, else `None`
    pub fn add_snap(&mut self, snap_diff: f64, timestamp: Duration) {
        let ping_avg = self.ping_average().as_secs_f64() / 2.0;
        let ping_min = self.ping_min().as_secs_f64() / 2.0;
        let ping_max = self.ping_max().as_secs_f64() / 2.0;

        // check how far off this snap diff is
        // if the snap is outside of the ping jitter, we have to assume a lag
        // either by network or bcs the client clock runs behind/fore
        let lag_weight = if snap_diff > 0.0 && snap_diff > (ping_max - ping_avg) {
            Some(snap_diff - (ping_max - ping_avg))
        } else if snap_diff < 0.0 && snap_diff.abs() > (ping_avg - ping_min) {
            Some((ping_avg - ping_min) - snap_diff.abs())
        } else {
            None
        };

        let cur_duration_snap = self.cur_duration_snap;
        if (timestamp > cur_duration_snap + self.calc_snap_farsight_of_jitter())
            || lag_weight.is_some()
        {
            self.cur_duration_snap = timestamp;
            let (last_snaps_average, weight) = self.last_snaps_average;
            let mut adjust_factor = last_snaps_average / weight as f64;
            if let Some(lag_weight) = lag_weight {
                /*dbg!(
                    lag_weight * 1000.0,
                    snap_diff * 1000.0,
                    ping_min * 1000.0,
                    ping_max * 1000.0,
                    ping_avg * 1000.0,
                );*/
                adjust_factor += lag_weight;
                self.last_snaps_average = (0.0, 1);
            } else {
                self.last_snaps_average = (snap_diff, 1);
            }
            /*let dist_ratio = Self::dist_ratio(
                adjust_factor,
                ping_min.as_secs_f64(),
                ping_max.as_secs_f64(),
                ping_avg.as_secs_f64(),
            );*/
            self.smooth_time = adjust_factor;
        } else {
            let (cur_average, avg_count) = &mut self.last_snaps_average;
            *avg_count += 1;
            *cur_average += snap_diff;
        }
    }

    pub fn smooth_time(&mut self) -> f64 {
        let frame_time = self.max_frametime().as_secs_f64().clamp(0.000001, f64::MAX) / 10.0;

        let res = self.smooth_time * frame_time;
        self.smooth_time *= 1.0 - frame_time;
        res
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use math::math::Rng;
    use textplots::{Chart, ColorPlot, Shape};

    use super::PredictionTiming;

    #[test]
    fn jitter_tests() {
        let ping_offset = 100.0 / 1000.0;
        let max_jitter = 600.0 / 1000.0;

        let mut cur_time = Duration::from_secs(1);
        let mut rng = Rng::new(0);
        let mut timer = PredictionTiming::new(
            Duration::from_secs_f64(ping_offset + max_jitter * 2.0),
            cur_time,
        );

        let mut time = cur_time - Duration::from_secs_f64(ping_offset / 2.0 + max_jitter);

        let mut chart_vals_time_diff = Vec::new();
        let mut chart_vals_max_ping = Vec::new();
        let mut chart_vals_avg_ping = Vec::new();
        let mut chart_vals_smooth_time = Vec::new();

        let snaps_per_sec = 20;
        let fps = 200;
        let snap_off = 2000;
        for i in 0..snap_off + 100 {
            cur_time = Duration::from_secs_f64(cur_time.as_secs_f64() + 1.0 / snaps_per_sec as f64);
            time = Duration::from_secs_f64(time.as_secs_f64() + 1.0 / snaps_per_sec as f64);

            let ratio = rng.random_float() as f64;
            let ratio_ping = rng.random_float() as f64 * 2.0;

            timer.add_ping(
                Duration::from_secs_f64(ping_offset + max_jitter * ratio_ping),
                cur_time,
            );

            let pred_max = timer.pred_max_smooth().as_secs_f64();

            let time_diff = cur_time.as_secs_f64() - time.as_secs_f64();
            let off = time_diff - (ping_offset / 2.0 + max_jitter * ratio) - pred_max;

            timer.add_snap(off, cur_time);

            let smooth_time = timer.smooth_time;
            // simulate multiple frames (FPS) per predicition timer adjustment

            for _ in 0..(fps / snaps_per_sec) {
                timer.add_frametime(Duration::from_secs_f64(1.0 / fps as f64), cur_time);

                time = Duration::from_secs_f64(
                    (time.as_secs_f64() + timer.smooth_time()).clamp(0.0, f64::MAX),
                );
            }

            dbg!(
                smooth_time * 1000.0,
                off * 1000.0,
                time_diff * 1000.0,
                time,
                pred_max * 1000.0,
                timer.ping_max().as_secs_f64() * 1000.0,
                timer.ping_average().as_secs_f64() * 1000.0,
                max_jitter * ratio_ping,
                timer.smooth_max_ping,
                timer.max_frametime().as_secs_f64() * 1000.0,
                timer.jitter_range,
            );
            if i >= snap_off {
                chart_vals_time_diff.push(time_diff * 1000.0);
                chart_vals_max_ping.push(timer.ping_max().as_secs_f64() / 2.0 * 1000.0);
                chart_vals_avg_ping.push(timer.ping_average().as_secs_f64() * 1000.0);
                chart_vals_smooth_time.push(smooth_time * 1000.0);
            }
        }
        let mut chart = Chart::new(
            chart_vals_time_diff.len() as u32 * 2,
            600,
            0.0,
            chart_vals_time_diff.len() as f32,
        );
        let shape = Shape::Continuous(Box::new(|x| {
            chart_vals_time_diff[x
                .round()
                .clamp(0.0, chart_vals_time_diff.len() as f32 - 1.0)
                as usize] as f32
        }));
        let shape_ping = Shape::Continuous(Box::new(|x| {
            chart_vals_max_ping
                [x.round().clamp(0.0, chart_vals_max_ping.len() as f32 - 1.0) as usize]
                as f32
        }));
        let shape_avg_ping = Shape::Continuous(Box::new(|x| {
            chart_vals_avg_ping
                [x.round().clamp(0.0, chart_vals_avg_ping.len() as f32 - 1.0) as usize]
                as f32
        }));
        let shape_smooth_time = Shape::Continuous(Box::new(|x| {
            chart_vals_smooth_time[x
                .round()
                .clamp(0.0, chart_vals_smooth_time.len() as f32 - 1.0)
                as usize] as f32
        }));
        let show_time_diff = true;
        let show_time_ping_half = true;
        let show_time_avg_ping = true;
        let show_time_smooth_time = true;
        let mut chart = &mut chart;

        if show_time_diff {
            chart = chart.linecolorplot(&shape, rgb::RGB8::new(255, 0, 0));
        }
        if show_time_ping_half {
            chart = chart.linecolorplot(&shape_ping, rgb::RGB8::new(0, 255, 0));
        }
        if show_time_avg_ping {
            chart = chart.linecolorplot(&shape_avg_ping, rgb::RGB8::new(0, 0, 255));
        }
        if show_time_smooth_time {
            chart = chart.linecolorplot(&shape_smooth_time, rgb::RGB8::new(255, 0, 255));
        }
        chart.y_axis();
        chart.x_axis();
        chart.display();
    }
}
