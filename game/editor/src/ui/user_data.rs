use std::path::PathBuf;

use base_io::io::IO;
use config::config::ConfigEngine;
use egui_file_dialog::FileDialog;
use graphics::handles::{
    canvas::canvas::GraphicsCanvasHandle, stream::stream::GraphicsStreamHandle,
};

use crate::{
    tab::EditorTab,
    tools::{tile_layer::auto_mapper::TileLayerAutoMapper, tool::Tools},
};

pub enum EditorUiEvent {
    OpenFile {
        name: PathBuf,
    },
    SaveFile {
        name: PathBuf,
    },
    HostMap {
        map_path: PathBuf,
        port: u16,
        password: String,
        pub_key_der: Vec<u8>,
        private_key_der: Vec<u8>,
    },
    Join {
        ip_port: String,
        cert_hash: String,
        password: String,
    },
}

pub enum EditorMenuHostDialogMode {
    SelectMap {
        file_dialog: FileDialog,
    },
    HostNetworkOptions {
        map_path: PathBuf,
        port: u16,
        password: String,
        pub_key_der: Vec<u8>,
        private_key_der: Vec<u8>,
    },
}

pub enum EditorMenuDialogMode {
    None,
    Open {
        file_dialog: FileDialog,
    },
    Save {
        file_dialog: FileDialog,
    },
    Host {
        mode: EditorMenuHostDialogMode,
    },
    Join {
        ip_port: String,
        cert_hash: String,
        password: String,
    },
}

impl EditorMenuDialogMode {
    pub fn open(io: &IO) -> Self {
        let mut open_path = io.fs.get_save_path();
        open_path.push("map/maps");

        let mut file_dialog = FileDialog::new()
            .title("Open Map File")
            .movable(false)
            .initial_directory(open_path)
            .default_file_name("ctf1.twmap");

        file_dialog.select_file();

        Self::Open { file_dialog }
    }
    pub fn save(io: &IO) -> Self {
        let mut open_path = io.fs.get_save_path();
        open_path.push("map/maps");

        let mut file_dialog = FileDialog::new()
            .title("Save Map File")
            .movable(false)
            .initial_directory(open_path)
            .default_file_name("ctf1.twmap");

        file_dialog.save_file();

        Self::Save { file_dialog }
    }
    pub fn host(io: &IO) -> Self {
        let mut open_path = io.fs.get_save_path();
        open_path.push("map/maps");

        let mut file_dialog = FileDialog::new()
            .title("Map File to host")
            .movable(false)
            .initial_directory(open_path)
            .default_file_name("ctf1.twmap");

        file_dialog.select_file();

        Self::Host {
            mode: EditorMenuHostDialogMode::SelectMap { file_dialog },
        }
    }
    pub fn join() -> Self {
        Self::Join {
            ip_port: Default::default(),
            cert_hash: Default::default(),
            password: Default::default(),
        }
    }
}

pub struct UserData<'a> {
    pub ui_events: &'a mut Vec<EditorUiEvent>,
    pub config: &'a ConfigEngine,
    pub editor_tab: Option<&'a mut EditorTab>,
    pub canvas_handle: &'a GraphicsCanvasHandle,
    pub stream_handle: &'a GraphicsStreamHandle,
    pub unused_rect: &'a mut Option<egui::Rect>,
    pub menu_dialog_mode: &'a mut EditorMenuDialogMode,
    pub tools: &'a mut Tools,
    pub auto_mapper: &'a mut TileLayerAutoMapper,
    pub pointer_is_used: &'a mut bool,
    pub io: &'a IO,
}

pub struct UserDataWithTab<'a> {
    pub ui_events: &'a mut Vec<EditorUiEvent>,
    pub config: &'a ConfigEngine,
    pub canvas_handle: &'a GraphicsCanvasHandle,
    pub stream_handle: &'a GraphicsStreamHandle,
    pub editor_tab: &'a mut EditorTab,
    pub tools: &'a mut Tools,
    pub pointer_is_used: &'a mut bool,
    pub io: &'a IO,
}
