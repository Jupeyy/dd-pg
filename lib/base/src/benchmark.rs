use std::sync::atomic::{AtomicU64, AtomicUsize};

// TODO: make this whole module optional, globals are not allowed in non wasm code
static START_TIME: once_cell::sync::Lazy<std::time::Instant> =
    once_cell::sync::Lazy::new(|| std::time::Instant::now());
static CALL_STACK_COUNT: once_cell::sync::Lazy<AtomicUsize> =
    once_cell::sync::Lazy::new(|| AtomicUsize::new(0));

pub struct Benchmark {
    is_active: bool,

    start_time: Option<std::time::Instant>,
    cur_diff: AtomicU64,
    offset: usize,
}

impl Benchmark {
    pub fn new(do_bench: bool) -> Self {
        let (start_time, cur_diff) = if do_bench {
            (Some(std::time::Instant::now()), AtomicU64::new(0))
        } else {
            (None, AtomicU64::new(0))
        };

        let offset = if do_bench {
            let _ = *START_TIME;
            CALL_STACK_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        } else {
            0
        };

        Self {
            is_active: do_bench,

            cur_diff,
            start_time,
            offset,
        }
    }

    /// does not overwrite current time
    pub fn bench_multi(&self, name: &str) -> u64 {
        if self.is_active {
            let cur_diff = self.cur_diff.load(std::sync::atomic::Ordering::SeqCst);
            let diff = self.start_time.unwrap().elapsed().as_millis() as u64 - cur_diff;
            let tabs: String = (0..self.offset)
                .map(|_| "  ")
                .collect::<Vec<&str>>()
                .join("");
            println!(
                "{}{} took {:.2}s / {:.2}ms / {:.2}ms global",
                tabs,
                name,
                diff as f64 / 1000.0,
                diff,
                START_TIME.elapsed().as_millis()
            );
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

impl Drop for Benchmark {
    fn drop(&mut self) {
        if self.is_active {
            CALL_STACK_COUNT.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
}
