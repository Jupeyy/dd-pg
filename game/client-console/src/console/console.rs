use base::system::{self, SystemTimeInterface};
use client_render::generic_ui_renderer;
use client_types::console::{entries_to_parser, ConsoleEntry};
use client_ui::console::{page::ConsoleUi, user_data::UserData};
use command_parser::parser::parse;
use config::config::ConfigEngine;
use egui::Color32;
use game_config::config::{Config, ConfigGame};
use graphics::graphics::graphics::Graphics;
use ui_base::{
    types::UiRenderPipe,
    ui::{UiContainer, UiCreator},
};

pub struct ConsoleRenderPipe<'a> {
    pub graphics: &'a Graphics,
    pub sys: &'a system::System,
    pub config: &'a mut Config,
    pub msgs: &'a mut String,
}

pub trait ConsoleEvents<E> {
    fn take(&self) -> Vec<E>;
}

pub struct ConsoleRender<E, T> {
    pub ui: UiContainer,
    pub entries: Vec<ConsoleEntry>,
    pub text: String,
    pub cursor: usize,
    pub selected_index: Option<usize>,
    pub console_ui: ConsoleUi,

    console_events: Box<dyn ConsoleEvents<E>>,
    pub user: T,
}

impl<E, T> ConsoleRender<E, T> {
    pub fn new(
        creator: &UiCreator,
        entries: Vec<ConsoleEntry>,
        console_events: Box<dyn ConsoleEvents<E>>,
        bg_color: Color32,
        user: T,
    ) -> Self {
        let mut ui = UiContainer::new(creator);
        ui.set_main_panel_color(&Color32::TRANSPARENT);

        Self {
            ui,
            entries,
            text: Default::default(),
            selected_index: None,
            cursor: 0,
            console_ui: ConsoleUi::new(bg_color),
            console_events,
            user,
        }
    }

    pub fn parse_cmd(
        &self,
        cmd: &str,
        config_game: &mut ConfigGame,
        config_engine: &mut ConfigEngine,
    ) {
        if !cmd.is_empty() {
            let cmds = parse(cmd, &entries_to_parser(&self.entries));
            client_ui::console::utils::run_commands(
                &cmds,
                &self.entries,
                config_engine,
                config_game,
                &mut String::new(),
            );
        }
    }

    #[must_use]
    pub fn render(
        &mut self,
        inp: egui::RawInput,
        pipe: &mut ConsoleRenderPipe,
    ) -> (Vec<E>, egui::PlatformOutput) {
        let mut user_data = UserData {
            entries: &self.entries,
            msgs: pipe.msgs,
            msg: &mut self.text,
            cursor: &mut self.cursor,
            select_index: &mut self.selected_index,
            config: pipe.config,
        };
        let mut ui_pipe = UiRenderPipe::new(pipe.sys.time_get_nanoseconds(), &mut user_data);

        let res = generic_ui_renderer::render(
            &pipe.graphics.backend_handle,
            &pipe.graphics.texture_handle,
            &pipe.graphics.stream_handle,
            &pipe.graphics.canvas_handle,
            &mut self.ui,
            &mut self.console_ui,
            &mut ui_pipe,
            inp.clone(),
            inp,
        );

        (self.get_events(), res)
    }

    #[must_use]
    pub fn get_events(&self) -> Vec<E> {
        self.console_events.take()
    }
}
