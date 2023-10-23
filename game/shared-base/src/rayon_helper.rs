#[macro_export]
macro_rules! join_all {
    ($e1:expr, $e2:expr) => {
        rayon::join($e1, $e2)
    };
    ($e1:expr, $e2:expr, $e3:expr) => {{
        let ((a, b), c) = rayon::join(|| join_all!($e1, $e2), $e3);
        (a, b, c)
    }};
    ($e1:expr, $e2:expr, $e3:expr, $e4:expr) => {{
        let ((a, b), (c, d)) = rayon::join(|| join_all!($e1, $e2), || join_all!($e3, $e4));
        (a, b, c, d)
    }};
    ($e1:expr, $e2:expr, $e3:expr, $e4:expr, $e5:expr) => {{
        let ((a, b, c, d), e) = rayon::join(|| join_all!($e1, $e2, $e3, $e4), $e5);
        (a, b, c, d, e)
    }};
    ($e1:expr, $e2:expr, $e3:expr, $e4:expr, $e5:expr, $e6:expr) => {{
        let ((a, b, c, d, e), f) = rayon::join(|| join_all!($e1, $e2, $e3, $e4, $e5), $e6);
        (a, b, c, d, e, f)
    }};
    ($e1:expr, $e2:expr, $e3:expr, $e4:expr, $e5:expr, $e6:expr, $e7:expr) => {{
        let ((a, b, c, d, e, f), g) = rayon::join(|| join_all!($e1, $e2, $e3, $e4, $e5, $e6), $e7);
        (a, b, c, d, e, f, g)
    }};
    ($e1:expr, $e2:expr, $e3:expr, $e4:expr, $e5:expr, $e6:expr, $e7:expr, $e8:expr) => {{
        let ((a, b, c, d, e, f, g), h) =
            rayon::join(|| join_all!($e1, $e2, $e3, $e4, $e5, $e6, $e7), $e8);
        (a, b, c, d, e, f, g, h)
    }};
}
