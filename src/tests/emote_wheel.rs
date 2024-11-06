use std::time::Duration;

use client_containers::utils::RenderGameContainers;
use client_render::emote_wheel::render::{EmoteWheelRender, EmoteWheelRenderPipe};
use client_render_base::render::tee::RenderTee;
use game_interface::types::character_info::NetworkSkinInfo;
use graphics::graphics::graphics::Graphics;
use ui_base::ui::UiCreator;

use super::utils::render_helper;

pub fn test_emote_wheel(
    graphics: &Graphics,
    creator: &UiCreator,
    containers: &mut RenderGameContainers,
    render_tee: &RenderTee,
    save_screenshot: impl Fn(&str),
) {
    let mut emote_wheel = EmoteWheelRender::new(graphics, creator);

    let mut time_offset = Duration::ZERO;
    let mut render = |base_name: &str| {
        let render_internal = |_i: u64, time_offset: Duration| {
            emote_wheel.render(&mut EmoteWheelRenderPipe {
                cur_time: &time_offset,
                input: &mut None,
                skin_container: &mut containers.skin_container,
                emoticons_container: &mut containers.emoticons_container,
                tee_render: render_tee,
                emoticons: &"".try_into().unwrap(),
                skin: &"".try_into().unwrap(),
                skin_info: &Some(NetworkSkinInfo::Original),
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

    render("emote_wheel_empty");
}
