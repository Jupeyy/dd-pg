use std::time::Duration;

use client_containers::utils::RenderGameContainers;
use client_render::vote::render::{VoteRender, VoteRenderPipe};
use client_render_base::render::tee::RenderTee;
use client_ui::vote::user_data::{VoteRenderData, VoteRenderPlayer, VoteRenderType};
use game_interface::votes::{MapVote, VoteState, VoteType, Voted};
use graphics::graphics::graphics::Graphics;
use ui_base::ui::UiCreator;

use super::utils::render_helper;

pub fn test_vote(
    graphics: &Graphics,
    creator: &UiCreator,
    containers: &mut RenderGameContainers,
    render_tee: &RenderTee,
    save_screenshot: impl Fn(&str),
) {
    let mut vote = VoteRender::new(graphics, creator);

    let mut time_offset = Duration::ZERO;
    let mut render = |base_name: &str| {
        let render_internal = |_i: u64, time_offset: Duration| {
            vote.render(&mut VoteRenderPipe {
                cur_time: &time_offset,
                skin_container: &mut containers.skin_container,
                tee_render: render_tee,
                vote_data: VoteRenderData {
                    ty: VoteRenderType::PlayerVoteKick(VoteRenderPlayer {
                        name: "nameless tee",
                        skin: &Default::default(),
                        skin_info: &Default::default(),
                    }),
                    data: &VoteState {
                        vote: VoteType::Map(MapVote {
                            name: "A_Map".try_into().unwrap(),
                            hash: Default::default(),
                            thumbnail_resource: false,
                        }),
                        remaining_time: Duration::ZERO,
                        yes_votes: 5,
                        no_votes: 4,
                        allowed_to_vote_count: 10,
                    },
                    remaining_time: &Duration::from_secs(1),
                    voted: Some(Voted::Yes),
                },
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

    render("vote_broken");
}
