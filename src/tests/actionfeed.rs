use std::{collections::VecDeque, time::Duration};

use client_containers::utils::RenderGameContainers;
use client_render::actionfeed::render::{ActionfeedRender, ActionfeedRenderPipe};
use client_render_base::render::{tee::RenderTee, toolkit::ToolkitRender};
use client_types::actionfeed::{Action, ActionInFeed, ActionKill, ActionPlayer};
use game_interface::{
    events::{GameWorldActionKillWeapon, KillFlags},
    types::character_info::NetworkSkinInfo,
};
use graphics::graphics::graphics::Graphics;
use math::math::vector::ubvec4;
use ui_base::{remember_mut::RememberMut, ui::UiCreator};

use super::utils::render_helper;

pub fn test_actionfeed(
    graphics: &Graphics,
    creator: &UiCreator,
    containers: &mut RenderGameContainers,
    render_tee: &RenderTee,
    toolkit_render: &ToolkitRender,
    save_screenshot: impl Fn(&str),
) {
    let mut actionfeed = ActionfeedRender::new(graphics, creator);

    let mut time_offset = Duration::ZERO;
    let mut render = |base_name: &str, msgs: &VecDeque<ActionInFeed>| {
        let render_internal = |_i: u64, time_offset: Duration| {
            actionfeed.msgs = RememberMut::new(msgs.clone());
            actionfeed.render(&mut ActionfeedRenderPipe {
                cur_time: &time_offset,
                skin_container: &mut containers.skin_container,
                tee_render: render_tee,
                weapon_container: &mut containers.weapon_container,
                toolkit_render,
                ninja_container: &mut containers.ninja_container,
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

    render("actionfeed_empty", &Default::default());
    let mut entries = vec![];
    for i in 0..5 {
        entries.push(ActionInFeed {
            action: Action::Kill(ActionKill {
                killer: Some(ActionPlayer {
                    name: if i % 2 == 0 {
                        "k".into()
                    } else {
                        "WWWWWWWWWWWWWWWWWWWWWWWW".into()
                    },
                    skin: Default::default(),
                    skin_info: NetworkSkinInfo::Original,
                    weapon: Default::default(),
                }),
                assists: vec![],
                victims: vec![ActionPlayer {
                    name: if i % 2 == 0 {
                        "v".into()
                    } else {
                        "WWWWWWWWWWWWWWWWWWWWWWWW".into()
                    },
                    skin: Default::default(),
                    skin_info: NetworkSkinInfo::Custom {
                        body_color: ubvec4::new(255, 255, 255, 255),
                        feet_color: ubvec4::new(255, 255, 255, 255),
                    },
                    weapon: Default::default(),
                }],
                weapon: GameWorldActionKillWeapon::Ninja,
                flags: KillFlags::empty(),
            }),
            add_time: Duration::MAX,
        });
    }
    render("actionfeed_short_long", &entries.into());
}
