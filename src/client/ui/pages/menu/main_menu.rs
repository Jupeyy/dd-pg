use std::sync::{Arc, Mutex};

use base_fs::{filesys::FileSystem, io_batcher::TokIOBatcher};
use graphics::graphics::Graphics;
use server::server::ServerInfo;
use ui_wasm_manager::{UIRenderCallbackFunc, UIWinitWrapper};

use ui_base::types::{UIPipe, UIState};

pub struct MainMenu {
    // attributes
    server_info: Arc<ServerInfo>,
}

impl MainMenu {
    pub fn new(server_info: Arc<ServerInfo>) -> Self {
        Self { server_info }
    }
}

impl UIRenderCallbackFunc for MainMenu {
    fn render(
        &mut self,
        ui: &mut egui::Ui,
        pipe: &mut UIPipe<UIWinitWrapper>,
        ui_state: &mut UIState<UIWinitWrapper>,
        graphics: &mut Graphics,
        fs: &Arc<FileSystem>,
        io_batcher: &Arc<Mutex<TokIOBatcher>>,
    ) {
        if ui.button("tee editor").clicked() {
            pipe.ui_feedback.call_path(pipe.config, "editor", "tee");
        }
        if ui.button("color demo").clicked() {
            pipe.ui_feedback.call_path(pipe.config, "", "color");
        }
        if ui.button("size demo").clicked() {
            pipe.ui_feedback.call_path(pipe.config, "", "demo");
        }
        if ui.button("wasm demo").clicked() {
            pipe.ui_feedback.call_path(pipe.config, "wasm", "wasm");
        }
        if ui.button("LAN server").clicked() {
            let server_addr = self.server_info.sock_addr.lock().unwrap().clone();
            match server_addr {
                Some(addr) => {
                    pipe.config.ui_last_server_addr =
                        "127.0.0.1:".to_string() + &addr.port().to_string()
                }
                None => {}
            }
        }
        if ui.button("Connect to server").clicked() {
            pipe.ui_feedback
                .network_connect(&pipe.config.ui_last_server_addr);
            ui_state.is_ui_open = false;
        }
        if ui.button("Disconnect from server").clicked() {
            pipe.ui_feedback.network_disconnect();
        }
        ui.horizontal(|ui| {
            ui.label("Server addr: ");
            ui.text_edit_singleline(&mut pipe.config.ui_last_server_addr);
        });
    }

    fn destroy(self: Box<Self>, graphics: &mut Graphics) {}
}
