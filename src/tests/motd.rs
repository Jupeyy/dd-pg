use std::time::Duration;

use client_render::motd::page::{MotdRender, MotdRenderPipe};
use graphics::graphics::graphics::Graphics;
use ui_base::ui::UiCreator;

use super::utils::render_helper;

pub fn test_motd(graphics: &Graphics, creator: &UiCreator, save_screenshot: impl Fn(&str)) {
    let mut motd = MotdRender::new(graphics, creator);

    let mut time_offset = Duration::ZERO;
    let mut render = |base_name: &str, msg: &str| {
        let render_internal = |_i: u64, time_offset: Duration| {
            motd.msg = msg.into();
            motd.started_at = Some(time_offset);
            motd.render(&mut MotdRenderPipe {
                cur_time: &time_offset,
            });
        };
        render_helper(
            graphics,
            render_internal,
            &mut time_offset,
            base_name,
            &save_screenshot,
        );
    };

    render("motd_empty", "");
    render("motd_short", "abc");
    render(
        "motd_long",
        "hello hello \
        wwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwww\
        wwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwww\
        wwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwww\
        wwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwww",
    );
    render(
        "motd_commonmark",
        "# hello\n\
        __how are you__\n\
        `i am fine`\
        ",
    );
}
