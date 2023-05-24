use graphics::graphics::Graphics;
use network::network::quinn_network::QuinnNetwork;

use crate::ui::{
    pages::{
        demo::demo_page,
        editor::tee::{TeeEditor, TeeEditorPipe},
        test::ColorTest,
    },
    types::{UIFeedbackInterface, UIPipe, UIState},
};

pub struct MainMenu {
    // attributes
    connect_addr: String,

    tee_editor: TeeEditor,

    color_test: ColorTest,
}

impl MainMenu {
    pub fn new(graphics: &mut Graphics) -> Self {
        Self {
            connect_addr: "127.0.0.1:8305".to_string(),

            tee_editor: TeeEditor::new(graphics),
            color_test: ColorTest::default(),
        }
    }

    pub fn render_func(&mut self, ui: &mut egui::Ui, pipe: &mut UIPipe, ui_state: &mut UIState) {
        match pipe.config.ui_path.name.as_str() {
            "" => {
                if ui.button("tee editor").clicked() {
                    pipe.config.ui_path.route("editor/tee");
                    pipe.config.save();
                }
                if ui.button("color demo").clicked() {
                    pipe.config.ui_path.route("color");
                    pipe.config.save();
                }
                if ui.button("size demo").clicked() {
                    pipe.config.ui_path.route("demo");
                    pipe.config.save();
                }
                if ui.button("Connect to server").clicked() {
                    pipe.ui_feedback.network_connect(&self.connect_addr);
                    ui_state.is_ui_open = false;
                }
                if ui.button("Disconnect from server").clicked() {
                    pipe.ui_feedback.network_disconnect();
                }
                ui.horizontal(|ui| {
                    ui.label("Server addr: ");
                    ui.text_edit_singleline(&mut self.connect_addr);
                });
            }
            "editor/tee" => {
                self.tee_editor.tee_editor_page(
                    ui,
                    &mut TeeEditorPipe {
                        graphics: pipe.graphics,
                        sys: &pipe.sys.time,
                        config: pipe.config,
                    },
                    ui_state,
                );
            }
            "color" => {
                self.color_test.ui(ui);
            }
            "demo" => {
                demo_page(ui);
            }
            _ => {}
        }
    }
}

pub struct MainMenuUIFeedback<'a> {
    network: &'a mut QuinnNetwork,
}

impl<'a> MainMenuUIFeedback<'a> {
    pub fn new(network: &'a mut QuinnNetwork) -> Self {
        Self { network: network }
    }
}

impl<'a> UIFeedbackInterface for MainMenuUIFeedback<'a> {
    fn network_connect(&mut self, addr: &str) {
        self.network.connect(addr);
    }

    fn network_disconnect(&mut self) {
        self.network
            .disconnect(&self.network.get_current_connect_id());
    }
}
