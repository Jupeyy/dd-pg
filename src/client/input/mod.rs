use std::{collections::HashMap, sync::Arc};

use base_fs::{filesys::FileSystem, io_batcher::TokIOBatcher};
use config::config::Config;
use graphics_backend::types::Graphics;
use math::math::{length, normalize_pre_length, vector::dvec2};

use native::input::binds::{BindKey, Binds};
use shared_base::binds::{BindActions, BindActionsHotkey, BindActionsLocalPlayer};
use ui_base::ui::UI;
use ui_wasm_manager::UIWinitWrapper;
use winit::{
    event::{DeviceId, MouseButton},
    keyboard::KeyCode,
    window::Window,
};

use crate::localplayer::{ClientPlayer, LocalPlayers};

pub type DeviceToLocalPlayerIndex = HashMap<DeviceId, usize>;

pub struct InputPipe<'a> {
    pub local_players: &'a mut LocalPlayers,
    pub ui: &'a mut UI<UIWinitWrapper>,
    pub config: &'a Config,
    pub device_to_local_player: &'a DeviceToLocalPlayerIndex,
    pub global_binds: &'a mut Binds<BindActions>,
    pub graphics: &'a mut Graphics,
    pub fs: &'a Arc<FileSystem>,
    pub io_batcher: &'a TokIOBatcher,
}

pub struct InputHandling<'a> {
    pub pipe: InputPipe<'a>,
}

impl<'a> InputHandling<'a> {
    fn handle_binds(ui: &mut UI<UIWinitWrapper>, local_player: &mut ClientPlayer) {
        let input = &mut local_player.input.inp;
        let actions = local_player.binds.process(false);
        let mut dir = 0;
        let mut jump = false;
        let mut fire = false;
        let mut hook = false;
        if let Some(actions) = actions {
            for action in actions {
                match action {
                    BindActions::LocalPlayer(local_player_action) => match local_player_action {
                        BindActionsLocalPlayer::MoveLeft => dir -= 1,
                        BindActionsLocalPlayer::MoveRight => dir += 1,
                        BindActionsLocalPlayer::Jump => jump = true,
                        BindActionsLocalPlayer::Fire => fire = true,
                        BindActionsLocalPlayer::Hook => hook = true,
                        BindActionsLocalPlayer::OpenMenu => ui.ui_state.is_ui_open = true,
                    },
                    _ => {
                        todo!()
                    }
                }
            }
        }
        input.jump.set(jump);
        input.fire.set(fire);
        input.hook.set(hook);
        input.dir.set(dir.clamp(-1, 1));
    }

    fn handle_global_binds(
        global_binds: &mut Binds<BindActions>,
        graphics: &mut Graphics,

        fs: &Arc<FileSystem>,
        io_batcher: &TokIOBatcher,
    ) {
        let actions = global_binds.process(false);
        if let Some(actions) = actions {
            for action in actions {
                match action {
                    BindActions::Hotkeys(hotkey) => match hotkey {
                        BindActionsHotkey::Screenshot => {
                            let png = graphics.do_screenshot().unwrap();

                            let fs = fs.clone();
                            io_batcher.spawn_without_queue(async move {
                                fs.write_file("test.png", png).await?;
                                Ok(())
                            });
                        }
                    },
                    _ => {
                        todo!()
                    }
                }
            }
        }
    }

    fn get_max_mouse_distance(config: &Config) -> f64 {
        let camera_max_distance = 200.0;
        let follow_factor = config.inp.mouse_follow_factor as f64 / 100.0;
        let dead_zone = config.inp.mouse_deadzone as f64;
        let max_distance = config.inp.mouse_max_distance as f64;
        (if follow_factor != 0.0 {
            camera_max_distance / follow_factor + dead_zone
        } else {
            max_distance
        })
        .min(max_distance)
    }

    fn clamp_cursor(config: &Config, local_player: &mut ClientPlayer) {
        let mouse_max = Self::get_max_mouse_distance(config);
        let min_distance = config.inp.mouse_min_distance as f64;
        let mouse_min = min_distance;

        let cursor = local_player.input.inp.cursor.to_vec2();
        let mut mouse_distance = length(&cursor);
        if mouse_distance < 0.001 {
            local_player
                .input
                .inp
                .cursor
                .from_vec2(&dvec2::new(0.001, 0.0));
            mouse_distance = 0.001;
        }
        if mouse_distance < mouse_min {
            local_player
                .input
                .inp
                .cursor
                .from_vec2(&(normalize_pre_length(&cursor, mouse_distance) * mouse_min));
        }
        let cursor = local_player.input.inp.cursor.to_vec2();
        mouse_distance = length(&cursor);
        if mouse_distance > mouse_max {
            local_player
                .input
                .inp
                .cursor
                .from_vec2(&(normalize_pre_length(&cursor, mouse_distance) * mouse_max));
        }
    }
}

impl<'a> InputHandling<'a> {
    pub fn key_down(&mut self, device: &DeviceId, key: &KeyCode) {
        if self
            .pipe
            .device_to_local_player
            .get(device)
            .copied()
            .unwrap_or(0)
            < self.pipe.local_players.len()
            || self.pipe.local_players.len() == 1
        {
            let local_player = self.pipe.local_players.values_mut().next().unwrap();
            local_player.binds.handle_key_down(&BindKey::Key(*key));
            Self::handle_binds(self.pipe.ui, local_player);
        }
        self.pipe.global_binds.handle_key_down(&BindKey::Key(*key));
        Self::handle_global_binds(
            self.pipe.global_binds,
            self.pipe.graphics,
            self.pipe.fs,
            self.pipe.io_batcher,
        );
    }

    pub fn key_up(&mut self, device: &DeviceId, key: &KeyCode) {
        if self
            .pipe
            .device_to_local_player
            .get(device)
            .copied()
            .unwrap_or(0)
            < self.pipe.local_players.len()
            || self.pipe.local_players.len() == 1
        {
            let local_player = self.pipe.local_players.values_mut().next().unwrap();
            local_player.binds.handle_key_up(&BindKey::Key(*key));
            Self::handle_binds(self.pipe.ui, local_player);
        }
        self.pipe.global_binds.handle_key_down(&BindKey::Key(*key));
        Self::handle_global_binds(
            self.pipe.global_binds,
            self.pipe.graphics,
            self.pipe.fs,
            self.pipe.io_batcher,
        );
    }

    pub fn mouse_down(&mut self, device: &DeviceId, _x: f64, _y: f64, btn: &MouseButton) {
        if self
            .pipe
            .device_to_local_player
            .get(device)
            .copied()
            .unwrap_or(0)
            < self.pipe.local_players.len()
            || self.pipe.local_players.len() == 1
        {
            let local_player = self.pipe.local_players.values_mut().next().unwrap();
            local_player.binds.handle_key_down(&BindKey::Mouse(*btn));
            Self::handle_binds(self.pipe.ui, local_player);
        }
        self.pipe
            .global_binds
            .handle_key_down(&BindKey::Mouse(*btn));
        Self::handle_global_binds(
            self.pipe.global_binds,
            self.pipe.graphics,
            self.pipe.fs,
            self.pipe.io_batcher,
        );
    }

    pub fn mouse_up(&mut self, device: &DeviceId, _x: f64, _y: f64, btn: &MouseButton) {
        if self
            .pipe
            .device_to_local_player
            .get(device)
            .copied()
            .unwrap_or(0)
            < self.pipe.local_players.len()
            || self.pipe.local_players.len() == 1
        {
            let local_player = self.pipe.local_players.values_mut().next().unwrap();
            local_player.binds.handle_key_up(&BindKey::Mouse(*btn));
            Self::handle_binds(self.pipe.ui, local_player);
        }
        self.pipe
            .global_binds
            .handle_key_down(&BindKey::Mouse(*btn));
        Self::handle_global_binds(
            self.pipe.global_binds,
            self.pipe.graphics,
            self.pipe.fs,
            self.pipe.io_batcher,
        );
    }

    pub fn mouse_move(&mut self, device: &DeviceId, _x: f64, _y: f64, xrel: f64, yrel: f64) {
        let factor = self.pipe.config.inp.mouse_sensitivity as f64 / 100.0;

        // TODO: for spec Factor *= m_pClient->m_Camera.m_Zoom;

        if self
            .pipe
            .device_to_local_player
            .get(device)
            .copied()
            .unwrap_or(0)
            < self.pipe.local_players.len()
            || self.pipe.local_players.len() == 1
        {
            let local_player = self.pipe.local_players.values_mut().next().unwrap();
            // TODO: [( device as usize).clamp(0, self.pipe.local_players.len())];
            let cur = local_player.input.inp.cursor.to_vec2();
            local_player
                .input
                .inp
                .cursor
                .from_vec2(&(cur + dvec2::new(xrel, yrel) * factor));
            Self::clamp_cursor(self.pipe.config, local_player);
        }
    }

    pub fn raw_event(&mut self, _window: &Window, event: &winit::event::WindowEvent) -> bool {
        if self.pipe.ui.ui_state.is_ui_open {
            self.pipe
                .ui
                .ui_state
                .native_state
                .state
                .on_event(&self.pipe.ui.egui_ctx, event)
                .consumed
        } else {
            false
        }
    }
}
