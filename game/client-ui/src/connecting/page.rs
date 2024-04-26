use game_config::config::Config;
use ui_base::types::{UIPipe, UIState};
use ui_traits::traits::UIRenderCallbackFunc;

use crate::events::UiEvents;

use super::{
    main_frame,
    user_data::{ConnectMode, UserData},
};

pub struct ConnectingUI {
    mode: ConnectMode,
    events: UiEvents,
}

impl ConnectingUI {
    pub fn new(mode: ConnectMode, events: UiEvents) -> Self {
        Self { mode, events }
    }

    fn render_impl(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<Config>,
        ui_state: &mut UIState,
        main_frame_only: bool,
    ) {
        main_frame::render(
            ui,
            &mut UIPipe {
                cur_time: pipe.cur_time,
                user_data: &mut UserData {
                    mode: &self.mode,
                    config: pipe.user_data,
                    events: &self.events,
                },
            },
            ui_state,
            main_frame_only,
        );
    }
}

impl<'a> UIRenderCallbackFunc<Config> for ConnectingUI {
    fn has_blur(&self) -> bool {
        true
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<Config>,
        ui_state: &mut UIState,
    ) {
        self.render_impl(ui, pipe, ui_state, true)
    }

    fn render(&mut self, ui: &mut egui::Ui, pipe: &mut UIPipe<Config>, ui_state: &mut UIState) {
        self.render_impl(ui, pipe, ui_state, false)
    }
}
