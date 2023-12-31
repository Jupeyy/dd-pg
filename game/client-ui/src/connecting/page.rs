use std::{cell::RefCell, rc::Rc};

use graphics::graphics::Graphics;
use ui_base::types::{UIPipe, UIState};
use ui_traits::traits::UIRenderCallbackFunc;

use super::{
    main_frame,
    user_data::{ConnectMode, UserData},
};

pub struct ConnectingUI {
    mode: Rc<RefCell<ConnectMode>>,
}

impl ConnectingUI {
    pub fn new(mode: Rc<RefCell<ConnectMode>>) -> Self {
        Self { mode }
    }

    fn render_impl(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<()>,
        ui_state: &mut UIState,
        graphics: &mut Graphics,
        main_frame_only: bool,
    ) {
        main_frame::render(
            ui,
            &mut UIPipe {
                ui_feedback: pipe.ui_feedback,
                cur_time: pipe.cur_time,
                config: pipe.config,
                user_data: UserData {
                    mode: &self.mode.borrow(),
                },
            },
            ui_state,
            graphics,
            main_frame_only,
        );
    }
}

impl<'a> UIRenderCallbackFunc<()> for ConnectingUI {
    fn has_blur(&self) -> bool {
        true
    }

    fn render_main_frame(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<()>,
        ui_state: &mut UIState,
        graphics: &mut Graphics,
    ) {
        self.render_impl(ui, pipe, ui_state, graphics, true)
    }

    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<()>,
        ui_state: &mut UIState,
        graphics: &mut Graphics,
    ) {
        self.render_impl(ui, pipe, ui_state, graphics, false)
    }
}
