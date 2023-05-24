#[macro_export]
macro_rules! benchmark {
    ($e1:expr, $e2:expr) => {
        if $e1 {
            $e2.time_get_nanoseconds()
        } else {
            Default::default()
        }
    };
    ($e1:expr, $e2:expr, $e3:expr, $e4:expr, $e5:expr) => {
        if $e1 {
            let diff = $e2.time_get_nanoseconds() - $e4;
            println!(
                "{} took {:.2}s / {:.2}ms",
                $e3,
                diff.as_secs_f32(),
                (diff.as_nanos() as f64) / (1000000.0 as f64)
            );
        }
    };
    ($e1:expr, $e2:expr, $e3:expr, $e4:expr) => {{
        let t = benchmark!($e1, $e2);
        let res = $e4();
        benchmark!($e1, $e2, $e3, t, false);
        res
    }};
    ($e1:expr, $e2:expr, $e3:expr, $e4:expr,) => {
        benchmark!($e1, $e2, $e3, $e4)
    };
}
