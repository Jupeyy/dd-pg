use std::sync::atomic::AtomicU64;

pub struct Benchmark {
    is_active: bool,

    start_time: Option<std::time::Instant>,
    cur_diff: AtomicU64,
}

impl Benchmark {
    pub fn new(do_bench: bool) -> Self {
        let (start_time, cur_diff) = if do_bench {
            (Some(std::time::Instant::now()), AtomicU64::new(0))
        } else {
            (None, AtomicU64::new(0))
        };

        Self {
            is_active: do_bench,

            cur_diff,
            start_time,
        }
    }

    /// does not overwrite current time
    pub fn bench_multi(&self, name: &str) -> u64 {
        if self.is_active {
            let cur_time = std::time::Instant::now();
            let cur_diff = self.cur_diff.load(std::sync::atomic::Ordering::SeqCst);
            let diff = cur_time
                .duration_since(self.start_time.unwrap())
                .as_millis() as u64
                - cur_diff;
            println!("{} took {:.2}s / {:.2}ms", name, diff as f64 / 1000.0, diff);
            diff + cur_diff
        } else {
            0
        }
    }

    pub fn bench(&self, name: &str) {
        if self.is_active {
            self.cur_diff
                .store(self.bench_multi(name), std::sync::atomic::Ordering::SeqCst);
        }
    }
}
