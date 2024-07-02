use std::{collections::VecDeque, thread::ThreadId};

use api::{graphics::graphics::GraphicsBackend, println, GRAPHICS, IO, RUNTIME_THREAD_POOL};
use api_ui_game::render::create_skin_container;
use base_io::{io_batcher::IoBatcherTask, yield_now::yield_now};
use client_containers_new::skins::SkinContainer;
use client_render_base::render::tee::RenderTee;
use client_types::{
    actionfeed::{ActionFeed, ActionFeedKill, ActionFeedKillWeapon, ActionFeedPlayer},
    chat::{ChatMsg, ChatMsgPlayerChannel, ServerMsg},
    console::ConsoleEntry,
    server_browser::{ServerBrowserData, ServerBrowserFilter, ServerBrowserServer},
};
use client_ui::console::user_data::UserData;
use game_interface::events::KillFeedFlags;
use graphics::{
    graphics::graphics::Graphics,
    handles::{canvas::canvas::GraphicsCanvasHandle, stream::stream::GraphicsStreamHandle},
};
use ui_base::types::{UiRenderPipe, UiState};
use ui_traits::traits::UiPageInterface;

pub struct ActionfeedPage {
    canvas_handle: GraphicsCanvasHandle,
    stream_handle: GraphicsStreamHandle,
    skin_container: SkinContainer,
    render_tee: RenderTee,
}

impl ActionfeedPage {
    pub fn new(graphics: &Graphics) -> Self {
        Self {
            canvas_handle: graphics.canvas_handle.clone(),
            stream_handle: graphics.stream_handle.clone(),
            skin_container: create_skin_container(),
            render_tee: RenderTee::new(graphics),
        }
    }

    fn render_impl(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<()>,
        ui_state: &mut UiState,
        main_frame_only: bool,
    ) {
        let mut entries = vec![];
        for i in 0..5 {
            entries.push(ActionFeed::Kill(ActionFeedKill {
                killer: Some(ActionFeedPlayer {
                    name: if i % 2 == 0 {
                        "k".into()
                    } else {
                        "WWWWWWWWWWWWWWWWWWWWWWWW".into()
                    },
                    skin: Default::default(),
                }),
                assists: vec![],
                victims: vec![ActionFeedPlayer {
                    name: if i % 2 == 0 {
                        "v".into()
                    } else {
                        "WWWWWWWWWWWWWWWWWWWWWWWW".into()
                    },
                    skin: Default::default(),
                }],
                weapon: ActionFeedKillWeapon::Ninja,
                flags: KillFeedFlags::empty(),
            }));
        }

        client_ui::actionfeed::main_frame::render(
            ui,
            &mut UiRenderPipe::new(
                pipe.cur_time,
                &mut client_ui::actionfeed::user_data::UserData {
                    canvas_handle: &self.canvas_handle,
                    stream_handle: &self.stream_handle,
                    entries: &entries.into(),
                    skin_container: &mut self.skin_container,
                    render_tee: &self.render_tee,
                },
            ),
            ui_state,
            main_frame_only,
        );
    }
}

impl UiPageInterface<()> for ActionfeedPage {
    fn has_blur(&self) -> bool {
        false
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UiRenderPipe<()>,
        ui_state: &mut UiState,
    ) {
        self.render_impl(ui, pipe, ui_state, true)
    }

    fn render(&mut self, ui: &mut egui::Ui, pipe: &mut UiRenderPipe<()>, ui_state: &mut UiState) {
        self.render_impl(ui, pipe, ui_state, false)
    }
}
