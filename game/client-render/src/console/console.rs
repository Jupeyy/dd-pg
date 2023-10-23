use base::system::{self, SystemTimeInterface};
use client_types::console::ConsoleEntry;
use client_ui::console::{page::ConsoleUI, user_data::UserData};
use config::{
    config::Config,
    traits::{ConfigInterface, ConfigValue},
};
use egui::Color32;
use egui_winit::State;
use graphics_backend::{backend::GraphicsBackend, types::Graphics};
use graphics_base_traits::traits::GraphicsSizeQuery;
use native::native::NativeImpl;
use shared_base::game_types::TGameElementID;
use ui_base::{
    types::{UIFeedbackInterface, UINativePipe, UIPipe},
    ui::{UIInterface, UI},
    ui_render::render_ui,
};
use ui_traits::traits::UIRenderCallbackFunc;
use ui_wasm_manager::{UIWinitWrapper, UIWinitWrapperDummyPipe, UIWinitWrapperPipe};

pub struct ConsoleRenderPipe<'a> {
    pub graphics: &'a mut Graphics,
    pub sys: &'a system::System,
    pub config: &'a mut Config,
    pub msgs: &'a mut String,
    pub window: &'a winit::window::Window,
    pub player_id: &'a TGameElementID,
}

pub struct ClientStatsUIFeedbackDummy {}

impl UIFeedbackInterface for ClientStatsUIFeedbackDummy {}

pub struct ConsoleRender {
    pub ui: UI<UIWinitWrapper>,
    pub entries: Vec<ConsoleEntry>,
    pub text: String,
    pub console_ui: ConsoleUI,
}

impl ConsoleRender {
    fn parse_conf_value_usage(val: ConfigValue) -> String {
        match val {
            ConfigValue::Struct { .. } => "unsupported".to_string(),
            ConfigValue::Int { min, max } => {
                "int [".to_string() + &min.to_string() + ".." + &max.to_string() + "]"
            }
            ConfigValue::Float { min, max } => {
                "float [".to_string() + &min.to_string() + ".." + &max.to_string() + "]"
            }
            ConfigValue::String {
                min_length,
                max_length,
            } => {
                "string length:[".to_string()
                    + &min_length.to_string()
                    + ".."
                    + &max_length.to_string()
                    + "]"
            }
            ConfigValue::StringOfList { allowed_values } => {
                "string in [".to_string() + &allowed_values.join(", ") + "]"
            }
            ConfigValue::Array { val_ty } => {
                "array [".to_string() + &Self::parse_conf_value_usage(*val_ty) + "]"
            }
            ConfigValue::JSONRecord { val_ty } => {
                "JSON-like { \"name\": \"".to_string()
                    + &Self::parse_conf_value_usage(*val_ty)
                    + "\" }"
            }
        }
    }

    fn parse_conf_values_as_str_list(
        cur_path: String,
        list: &mut Vec<ConsoleEntry>,
        val: ConfigValue,
    ) {
        match val {
            ConfigValue::Struct { attributes } => {
                for attribute in attributes {
                    let mut new_path = cur_path.clone();
                    if !cur_path.is_empty() {
                        new_path.push_str(".");
                    }
                    new_path.push_str(&attribute.name);
                    Self::parse_conf_values_as_str_list(new_path, list, attribute.val);
                }
            }
            _ => {
                list.push(ConsoleEntry {
                    full_name: cur_path,
                    usage: Self::parse_conf_value_usage(val),
                });
            }
        }
    }

    pub fn new(native: &mut dyn NativeImpl) -> Self {
        let mut ui = UI::new(
            UIWinitWrapper {
                state: State::new(native.borrow_window()),
            },
            None,
        );
        ui.set_main_panel_color(&Color32::TRANSPARENT);

        let val = Config::conf_value();
        let mut entries: Vec<ConsoleEntry> = Vec::new();
        Self::parse_conf_values_as_str_list("".to_string(), &mut entries, val);

        Self {
            ui,
            entries,
            text: Default::default(),
            console_ui: ConsoleUI {},
        }
    }

    fn render_impl(&mut self, pipe: &mut ConsoleRenderPipe, main_frame_only: bool) {
        let window_width = pipe.graphics.window_width();
        let window_height = pipe.graphics.window_height();
        let window_pixels_per_point = pipe.graphics.window_pixels_per_point();

        let input_generator = UIWinitWrapperPipe {
            window: pipe.window,
        };

        let mut ui_feedback = ClientStatsUIFeedbackDummy {};
        let mut ui_pipe = UIPipe::new(
            &mut ui_feedback,
            pipe.sys.time_get_nanoseconds(),
            pipe.config,
            UserData {
                entries: &self.entries,
                msgs: pipe.msgs,
                msg: &mut self.text,
            },
        );
        let mut native_ui_pipe = UINativePipe {
            raw_inp_generator: if main_frame_only {
                &UIWinitWrapperDummyPipe {}
            } else {
                &input_generator
            },
        };
        let (screen_rect, full_output, zoom_level) = self.ui.render(
            window_width,
            window_height,
            window_pixels_per_point,
            |ui, inner_pipe, ui_state| {
                if main_frame_only {
                    self.console_ui
                        .render_main_frame(ui, inner_pipe, ui_state, pipe.graphics)
                } else {
                    self.console_ui
                        .render(ui, inner_pipe, ui_state, pipe.graphics)
                }
            },
            &mut ui_pipe,
            &mut native_ui_pipe,
            main_frame_only,
        );
        render_ui(
            &mut self.ui,
            &mut native_ui_pipe,
            full_output,
            &screen_rect,
            zoom_level,
            &mut pipe.graphics,
            main_frame_only,
        );
    }

    pub fn render(&mut self, pipe: &mut ConsoleRenderPipe) {
        self.render_impl(pipe, false)
    }

    pub fn has_blur(&self) -> bool {
        <ConsoleUI as UIRenderCallbackFunc<UserData<'_>, GraphicsBackend>>::has_blur(
            &self.console_ui,
        )
    }

    pub fn render_main_frame(&mut self, pipe: &mut ConsoleRenderPipe) {
        self.render_impl(pipe, true)
    }
}
