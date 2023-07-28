use std::sync::{Arc, Mutex};

use base_fs::{filesys::FileSystem, io_batcher::TokIOBatcher};
use config::config::Config;
use network::network::quinn_network::QuinnNetwork;
use ui_base::types::UIFeedbackInterface;

pub mod main_menu;

pub struct ClientUIFeedback<'a> {
    network: &'a mut QuinnNetwork,
    fs: &'a Arc<FileSystem>,
    io_batcher: &'a Arc<Mutex<TokIOBatcher>>,
}

impl<'a> ClientUIFeedback<'a> {
    pub fn new(
        network: &'a mut QuinnNetwork,
        fs: &'a Arc<FileSystem>,
        io_batcher: &'a Arc<Mutex<TokIOBatcher>>,
    ) -> Self {
        Self {
            network,
            fs,
            io_batcher,
        }
    }
}

impl<'a> UIFeedbackInterface for ClientUIFeedback<'a> {
    fn network_connect(&mut self, addr: &str) {
        self.network.connect(addr);
    }

    fn network_disconnect(&mut self) {
        self.network
            .disconnect(&self.network.get_current_connect_id());
    }

    fn call_path(&mut self, config: &mut Config, mod_name: &str, path: &str) {
        if let Some(_) = mod_name.find(|c: char| !c.is_ascii_alphabetic()) {
            println!("Mod name must only contain ascii characters");
        } else {
            if let Some(_) = path.find(|c: char| !c.is_ascii_alphabetic()) {
                println!("Path name must only contain ascii characters");
            } else {
                config.ui_path.route(
                    &(if mod_name.is_empty() {
                        "".to_string()
                    } else {
                        mod_name.to_string() + "/"
                    } + path),
                );
                config_fs::save(config, self.fs, self.io_batcher);
            }
        }
    }
}
