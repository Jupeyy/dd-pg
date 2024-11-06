use std::time::Duration;

use graphics::graphics::graphics::Graphics;

pub fn render_helper(
    graphics: &Graphics,
    mut render_func: impl FnMut(u64, Duration),
    time_offset: &mut Duration,
    base_name: &str,
    save_screenshot: &dyn Fn(&str),
) {
    const RUNS: u64 = 5;
    const FAKE_FRAMES: u64 = 3;

    for i in 0..RUNS {
        // fake render for ui delayed rendering & centering
        for _ in 0..FAKE_FRAMES {
            render_func(i + 1, *time_offset);
            graphics.swap();
            *time_offset += Duration::from_secs(1);
        }
        render_func(i + 1, *time_offset);
        save_screenshot(&format!("{base_name}_{:0>3}", i));
    }
}
