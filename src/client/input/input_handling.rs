use std::collections::HashMap;

use base_io::io::Io;
use binds::binds::{
    gen_local_player_action_hash_map, BindActions, BindActionsHotkey, BindActionsLocalPlayer,
};
use client_render_base::map::render_tools::RenderTools;
use client_ui::console::utils::try_apply_config_val;
use client_ui::emote_wheel::user_data::EmoteWheelEvent;
use config::config::ConfigEngine;
use egui::{Context, CursorIcon};
use game_config::config::ConfigGame;
use game_interface::types::emoticons::EmoticonType;
use game_interface::types::render::character::{PlayerCameraMode, TeeEye};
use game_interface::types::{game::GameEntityId, input::CharacterInputCursor};
use graphics::graphics::graphics::{Graphics, ScreenshotCb};
use math::math::{length, normalize_pre_length, vector::dvec2};

use native::native::{DeviceId, MouseButton, MouseScrollDelta, PhysicalKey, Window};
use native::{
    input::binds::{BindKey, Binds, MouseExtra},
    native::NativeImpl,
};
use ui_base::{types::UiState, ui::UiContainer};

use crate::localplayer::{ClientPlayer, LocalPlayers};

pub type DeviceToLocalPlayerIndex = HashMap<DeviceId, usize>;

#[derive(Debug, Clone)]
pub struct InputKeyEv {
    key: BindKey,
    is_down: bool,
    device: DeviceId,
}

#[derive(Debug, Clone)]
pub struct InputAxisMoveEv {
    device: DeviceId,
    xrel: f64,
    yrel: f64,
}

#[derive(Debug, Clone)]
pub enum InputEv {
    Key(InputKeyEv),
    Move(InputAxisMoveEv),
}

impl InputEv {
    pub fn device(&self) -> &DeviceId {
        match self {
            InputEv::Key(ev) => &ev.device,
            InputEv::Move(ev) => &ev.device,
        }
    }
}

pub struct InputRes {
    pub egui: Option<egui::RawInput>,
}

struct Input {
    egui: Option<egui::RawInput>,
    evs: Vec<InputEv>,
}

impl Input {
    pub fn new() -> Self {
        Self {
            egui: Default::default(),
            evs: Default::default(),
        }
    }

    pub fn take(&mut self) -> InputRes {
        self.evs.clear();
        InputRes {
            egui: self.egui.take(),
        }
    }

    pub fn cloned(&mut self) -> InputRes {
        InputRes {
            egui: self.egui.clone(),
        }
    }
}

pub enum InputHandlingEvent {
    Kill {
        local_player_id: GameEntityId,
    },
    Emoticon {
        local_player_id: GameEntityId,
        emoticon: EmoticonType,
    },
    ChangeEyes {
        local_player_id: GameEntityId,
        eye: TeeEye,
    },
    VoteYes,
    VoteNo,
}

pub struct InputHandling {
    pub state: egui_winit::State,

    last_known_cursor: Option<CursorIcon>,

    inp: Input,

    bind_cmds: HashMap<&'static str, BindActionsLocalPlayer>,
}

impl InputHandling {
    pub fn new(window: &Window) -> Self {
        let ctx = Context::default();
        ctx.options_mut(|options| {
            options.zoom_with_keyboard = false;
        });

        let bind_cmds = gen_local_player_action_hash_map();
        Self {
            state: egui_winit::State::new(
                ctx,
                Default::default(),
                window,
                Some(window.scale_factor().clamp(0.00001, f64::MAX) as f32),
                None,
                None,
            ),
            last_known_cursor: None,
            inp: Input::new(),
            bind_cmds,
        }
    }

    pub fn new_frame(&mut self) {
        self.inp.take();
    }

    /// use this if you want to consume the input, all further calls will get `None` (for the current frame)
    pub fn take_inp(&mut self) -> InputRes {
        self.inp.take()
    }

    /// clone the input and leave it there for other components
    pub fn clone_inp(&mut self) -> InputRes {
        self.inp.cloned()
    }

    pub fn collect_events(&mut self) {
        self.inp.egui = Some(self.state.egui_input_mut().take());
    }

    pub fn set_last_known_cursor(&mut self, config: &ConfigEngine, cursor: CursorIcon) {
        if !config.inp.dbg_mode {
            self.last_known_cursor = Some(cursor);
        }
    }

    /// `apply_latest_known_cursor` is good if the ui that calls this
    /// actually doesn't have input focus right now
    pub fn handle_platform_output(
        &mut self,
        native: &mut dyn NativeImpl,
        mut platform_output: egui::PlatformOutput,
        apply_latest_known_cursor: bool,
    ) {
        if apply_latest_known_cursor {
            if let Some(cursor) = self.last_known_cursor {
                platform_output.cursor_icon = cursor;
            }
        }
        self.last_known_cursor = Some(platform_output.cursor_icon);
        native.toggle_cursor(!matches!(platform_output.cursor_icon, CursorIcon::None));
        self.state
            .handle_platform_output(native.borrow_window(), platform_output);
    }

    fn handle_binds_impl(
        ui: &mut UiContainer,
        local_player_id: &GameEntityId,
        local_player: &mut ClientPlayer,
        evs: &mut Vec<InputHandlingEvent>,
        config_engine: &mut ConfigEngine,
        config_game: &mut ConfigGame,
        bind_cmds: &HashMap<&'static str, BindActionsLocalPlayer>,
    ) {
        let input = &mut local_player.input.inp;
        let actions = local_player.binds.process();
        let mut dir = 0;
        let mut jump = false;
        let mut fire = false;
        let mut hook = false;
        let mut next_weapon = None;
        let mut next_show_scoreboard = false;
        let mut next_show_chat_all = false;
        let mut next_show_emote_wheel = false;
        let mut zoom_diff = Some(0);
        for actions in actions.press_actions.iter() {
            for action in actions {
                let mut handle_action = |action: &BindActionsLocalPlayer| match action {
                    BindActionsLocalPlayer::MoveLeft => dir -= 1,
                    BindActionsLocalPlayer::MoveRight => dir += 1,
                    BindActionsLocalPlayer::Jump => jump = true,
                    BindActionsLocalPlayer::Fire => fire = true,
                    BindActionsLocalPlayer::Hook => hook = true,
                    BindActionsLocalPlayer::NextWeapon => input.consumable.weapon_diff.add(1),
                    BindActionsLocalPlayer::PrevWeapon => input.consumable.weapon_diff.add(-1),
                    BindActionsLocalPlayer::Weapon(weapon) => {
                        next_weapon = Some(*weapon);
                    }
                    BindActionsLocalPlayer::ShowScoreboard => {
                        next_show_scoreboard = true;
                    }
                    BindActionsLocalPlayer::ShowChatHistory => {
                        next_show_chat_all = true;
                    }
                    BindActionsLocalPlayer::ShowEmoteWheel => {
                        next_show_emote_wheel = true;
                    }
                    BindActionsLocalPlayer::OpenMenu => {
                        // only listen for click
                    }
                    BindActionsLocalPlayer::ActivateChatInput => {
                        // only listen for click
                    }
                    BindActionsLocalPlayer::Kill => {
                        // only listen for click
                    }
                    BindActionsLocalPlayer::ToggleDummyCopyMoves => {
                        local_player.dummy_copy_moves = !local_player.dummy_copy_moves;
                    }
                    BindActionsLocalPlayer::ToggleDummyHammerFly => {
                        local_player.dummy_hammer = !local_player.dummy_hammer;
                    }
                    BindActionsLocalPlayer::VoteYes => {
                        // only listen for click
                    }
                    BindActionsLocalPlayer::VoteNo => {
                        // only listen for click
                    }
                    BindActionsLocalPlayer::ZoomOut => {
                        zoom_diff = zoom_diff.map(|diff| diff - 1);
                    }
                    BindActionsLocalPlayer::ZoomIn => {
                        zoom_diff = zoom_diff.map(|diff| diff + 1);
                    }
                    BindActionsLocalPlayer::ZoomReset => {
                        zoom_diff = None;
                    }
                };
                match action {
                    BindActions::LocalPlayer(action) => {
                        handle_action(action);
                    }
                    BindActions::Command(cmd) => {
                        if let Some(action) = bind_cmds.get(cmd.ident.as_str()) {
                            handle_action(action);
                        }
                    }
                }
            }
        }
        for actions in actions.click_actions.iter() {
            for action in actions {
                let mut handle_action = |action: &BindActionsLocalPlayer| match action {
                    BindActionsLocalPlayer::OpenMenu => {
                        if local_player.chat_input_active {
                            local_player.chat_input_active = false;
                        } else {
                            ui.ui_state.is_ui_open = true;
                        }
                    }
                    BindActionsLocalPlayer::ActivateChatInput => {
                        local_player.chat_input_active = true;
                    }
                    BindActionsLocalPlayer::Kill => evs.push(InputHandlingEvent::Kill {
                        local_player_id: *local_player_id,
                    }),
                    BindActionsLocalPlayer::VoteYes => {
                        evs.push(InputHandlingEvent::VoteYes);
                    }
                    BindActionsLocalPlayer::VoteNo => {
                        evs.push(InputHandlingEvent::VoteNo);
                    }
                    _ => {}
                };
                match action {
                    BindActions::LocalPlayer(action) => {
                        handle_action(action);
                    }
                    BindActions::Command(cmd) => {
                        if let Some(action) = bind_cmds.get(cmd.ident.as_str()) {
                            handle_action(action);
                        } else {
                            // TODO: show errors somewhere?
                            let _ = try_apply_config_val(
                                &cmd.cmd_text,
                                &cmd.args,
                                config_engine,
                                config_game,
                            );
                        }
                    }
                }
            }
        }
        if !*input.state.jump && jump {
            input.consumable.jump.add(1)
        }
        if !*input.state.fire && fire {
            input.consumable.fire.add(1, *input.cursor);
        }
        if !*input.state.hook && hook {
            input.consumable.hook.add(1, *input.cursor);
        }

        // generate emoticon/tee-eye event if needed
        if local_player.emote_wheel_active
            && !next_show_emote_wheel
            && local_player.last_emote_wheel_selection.is_some()
        {
            let ev = local_player.last_emote_wheel_selection.unwrap();
            match ev {
                EmoteWheelEvent::EmoticonSelected(emoticon) => {
                    evs.push(InputHandlingEvent::Emoticon {
                        local_player_id: *local_player_id,
                        emoticon,
                    });
                }
                EmoteWheelEvent::EyeSelected(eye) => {
                    evs.push(InputHandlingEvent::ChangeEyes {
                        local_player_id: *local_player_id,
                        eye,
                    });
                }
            }
        }
        local_player.emote_wheel_active = next_show_emote_wheel;

        input.state.jump.set(jump);
        input.state.fire.set(fire);
        input.state.hook.set(hook);
        input.state.dir.set(dir.clamp(-1, 1));
        input.consumable.set_weapon_req(next_weapon);
        local_player.show_scoreboard = next_show_scoreboard;
        local_player.show_chat_all = next_show_chat_all;
        local_player.zoom = zoom_diff
            .map(|diff| (local_player.zoom - diff as f32 * 0.1).clamp(0.01, 1024.0))
            .unwrap_or(1.0);
    }

    fn handle_global_binds_impl(
        global_binds: &mut Binds<BindActionsHotkey>,
        graphics: &Graphics,

        local_console_state: &mut UiState,
        mut remote_console_state: Option<&mut UiState>,
        debug_hud_state: &mut UiState,

        io: &Io,
    ) {
        let actions = global_binds.process();
        for action in actions.click_actions.iter() {
            match action {
                BindActionsHotkey::Screenshot => {
                    let io = io.clone();
                    #[derive(Debug)]
                    struct Screenshot {
                        io: Io,
                    }
                    impl ScreenshotCb for Screenshot {
                        fn on_screenshot(&self, png: anyhow::Result<Vec<u8>>) {
                            match png {
                                Ok(png) => {
                                    let fs = self.io.fs.clone();

                                    self.io.io_batcher.spawn_without_lifetime(async move {
                                        fs.write_file("test.png".as_ref(), png).await?;
                                        Ok(())
                                    });
                                }
                                Err(err) => {
                                    log::error!(target: "screenshot", "{err}");
                                }
                            }
                        }
                    }
                    graphics.do_screenshot(Screenshot { io }).unwrap();
                }
                BindActionsHotkey::LocalConsole => {
                    local_console_state.is_ui_open = !local_console_state.is_ui_open;
                    if let Some(remote_console_state) = remote_console_state.as_deref_mut() {
                        remote_console_state.is_ui_open = false;
                    }
                }
                BindActionsHotkey::RemoteConsole => {
                    if let Some(remote_console_state) = remote_console_state.as_deref_mut() {
                        remote_console_state.is_ui_open = !remote_console_state.is_ui_open;
                    }
                    local_console_state.is_ui_open = false;
                }
                BindActionsHotkey::ConsoleClose => {
                    local_console_state.is_ui_open = false;
                    if let Some(remote_console_state) = remote_console_state.as_deref_mut() {
                        remote_console_state.is_ui_open = false;
                    }
                }
                BindActionsHotkey::DebugHud => {
                    debug_hud_state.is_ui_open = !debug_hud_state.is_ui_open;
                }
            }
        }
    }

    fn get_max_mouse_distance(config: &ConfigGame) -> f64 {
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

    fn clamp_cursor(config: &ConfigGame, local_player: &mut ClientPlayer) {
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
                .set(CharacterInputCursor::from_vec2(&dvec2::new(0.001, 0.0)));
            mouse_distance = 0.001;
        }
        if mouse_distance < mouse_min {
            local_player
                .input
                .inp
                .cursor
                .set(CharacterInputCursor::from_vec2(
                    &(normalize_pre_length(&cursor, mouse_distance) * mouse_min),
                ));
        }
        let cursor = local_player.input.inp.cursor.to_vec2();
        mouse_distance = length(&cursor);
        if mouse_distance > mouse_max {
            local_player
                .input
                .inp
                .cursor
                .set(CharacterInputCursor::from_vec2(
                    &(normalize_pre_length(&cursor, mouse_distance) * mouse_max),
                ));
        }
    }

    pub fn handle_global_binds(
        &self,
        global_binds: &mut Binds<BindActionsHotkey>,
        local_console_ui: &mut UiContainer,
        mut remote_console_ui: Option<&mut UiContainer>,
        debug_hud_ui: &mut UiContainer,
        graphics: &Graphics,
        io: &Io,
    ) {
        for ev in &self.inp.evs {
            match ev {
                InputEv::Key(key_ev) => {
                    match &key_ev.key {
                        BindKey::Key(_) | BindKey::Mouse(_) => {
                            if key_ev.is_down {
                                global_binds.handle_key_down(&key_ev.key);
                            } else {
                                global_binds.handle_key_up(&key_ev.key);
                            }
                        }
                        BindKey::Extra(_) => {
                            global_binds.handle_key_down(&key_ev.key);
                            Self::handle_global_binds_impl(
                                global_binds,
                                graphics,
                                &mut local_console_ui.ui_state,
                                remote_console_ui.as_mut().map(|ui| &mut ui.ui_state),
                                &mut debug_hud_ui.ui_state,
                                io,
                            );
                            global_binds.handle_key_up(&key_ev.key);
                        }
                    }
                    Self::handle_global_binds_impl(
                        global_binds,
                        graphics,
                        &mut local_console_ui.ui_state,
                        remote_console_ui.as_mut().map(|ui| &mut ui.ui_state),
                        &mut debug_hud_ui.ui_state,
                        io,
                    );
                }
                InputEv::Move(_) => {}
            }
        }
    }

    /// returns a list of immediate events that are a result of a input
    pub fn handle_player_binds(
        &mut self,
        local_players: &mut LocalPlayers,
        ui: &mut UiContainer,
        device_to_local_player: &DeviceToLocalPlayerIndex,
        config_engine: &mut ConfigEngine,
        config_game: &mut ConfigGame,
        graphics: &Graphics,
    ) -> Vec<InputHandlingEvent> {
        let mut res = Vec::new();

        self.inp.evs.retain(|ev| {
            if device_to_local_player
                .get(ev.device())
                .copied()
                .unwrap_or(0)
                < local_players.len()
                || local_players.len() == 1
            {
                let (local_player_id, local_player) = local_players.iter_mut().next().unwrap();
                if !local_player.chat_input_active {
                    match ev {
                        InputEv::Key(key_ev) => match &key_ev.key {
                            BindKey::Key(_) | BindKey::Mouse(_) => {
                                if key_ev.is_down {
                                    local_player.binds.handle_key_down(&key_ev.key);
                                } else {
                                    local_player.binds.handle_key_up(&key_ev.key);
                                }
                                Self::handle_binds_impl(
                                    ui,
                                    local_player_id,
                                    local_player,
                                    &mut res,
                                    config_engine,
                                    config_game,
                                    &self.bind_cmds,
                                );
                            }
                            BindKey::Extra(_) => {
                                local_player.binds.handle_key_down(&key_ev.key);
                                Self::handle_binds_impl(
                                    ui,
                                    local_player_id,
                                    local_player,
                                    &mut res,
                                    config_engine,
                                    config_game,
                                    &self.bind_cmds,
                                );
                                local_player.binds.handle_key_up(&key_ev.key);
                            }
                        },
                        InputEv::Move(move_ev) => {
                            let factor = config_game.inp.mouse_sensitivity as f64 / 100.0;

                            // TODO: for spec Factor *= m_pClient->m_Camera.m_Zoom;

                            // TODO: [( device as usize).clamp(0, pipe.local_players.len())];
                            match local_player.input_cam_mode {
                                PlayerCameraMode::Default => {
                                    let cur = local_player.cursor_pos;
                                    local_player.input.inp.cursor.set(
                                        CharacterInputCursor::from_vec2(
                                            &(cur
                                                + dvec2::new(move_ev.xrel, move_ev.yrel) * factor),
                                        ),
                                    );
                                    Self::clamp_cursor(config_game, local_player);
                                    local_player.cursor_pos =
                                        local_player.input.inp.cursor.to_vec2();
                                }
                                PlayerCameraMode::Free => {
                                    let cur = local_player.free_cam_pos;

                                    let points = RenderTools::canvas_points_of_group(
                                        &graphics.canvas_handle,
                                        0.0,
                                        0.0,
                                        None,
                                        local_player.zoom,
                                    );
                                    let x_ratio = move_ev.xrel
                                        / graphics.canvas_handle.window_canvas_width() as f64;
                                    let y_ratio = move_ev.yrel
                                        / graphics.canvas_handle.window_canvas_height() as f64;

                                    let x = x_ratio * (points[2] as f64 - points[0] as f64);
                                    let y = y_ratio * (points[3] as f64 - points[1] as f64);

                                    let new = cur + dvec2::new(x, y);
                                    local_player
                                        .input
                                        .inp
                                        .cursor
                                        .set(CharacterInputCursor::from_vec2(&new));
                                    local_player.free_cam_pos = new;
                                }
                                PlayerCameraMode::LockedTo(_) => {
                                    // don't alter the cursor
                                }
                            }
                        }
                    }

                    false
                } else {
                    true
                }
            } else {
                true
            }
        });

        res
    }

    pub fn key_down(
        &mut self,
        _window: &native::native::Window,
        device: &DeviceId,
        key: &PhysicalKey,
    ) {
        self.inp.evs.push(InputEv::Key(InputKeyEv {
            key: BindKey::Key(*key),
            is_down: true,
            device: *device,
        }));
    }

    pub fn key_up(
        &mut self,
        _window: &native::native::Window,
        device: &DeviceId,
        key: &PhysicalKey,
    ) {
        self.inp.evs.push(InputEv::Key(InputKeyEv {
            key: BindKey::Key(*key),
            is_down: false,
            device: *device,
        }));
    }

    pub fn mouse_down(
        &mut self,
        _window: &native::native::Window,
        device: &DeviceId,
        _x: f64,
        _y: f64,
        btn: &MouseButton,
    ) {
        self.inp.evs.push(InputEv::Key(InputKeyEv {
            key: BindKey::Mouse(*btn),
            is_down: true,
            device: *device,
        }));
    }

    pub fn mouse_up(
        &mut self,
        _window: &native::native::Window,
        device: &DeviceId,
        _x: f64,
        _y: f64,
        btn: &MouseButton,
    ) {
        self.inp.evs.push(InputEv::Key(InputKeyEv {
            key: BindKey::Mouse(*btn),
            is_down: false,
            device: *device,
        }));
    }

    pub fn mouse_move(
        &mut self,
        _window: &native::native::Window,
        device: &DeviceId,
        _x: f64,
        _y: f64,
        xrel: f64,
        yrel: f64,
    ) {
        self.inp.evs.push(InputEv::Move(InputAxisMoveEv {
            device: *device,
            xrel,
            yrel,
        }))
    }

    pub fn scroll(
        &mut self,
        _window: &native::native::Window,
        device: &DeviceId,
        _x: f64,
        _y: f64,
        delta: &MouseScrollDelta,
    ) {
        let wheel_dir = {
            match delta {
                MouseScrollDelta::LineDelta(_, delta) => {
                    if *delta < 0.0 {
                        MouseExtra::WheelDown
                    } else {
                        MouseExtra::WheelUp
                    }
                }
                MouseScrollDelta::PixelDelta(delta) => {
                    if delta.y < 0.0 {
                        MouseExtra::WheelDown
                    } else {
                        MouseExtra::WheelUp
                    }
                }
            }
        };
        self.inp.evs.push(InputEv::Key(InputKeyEv {
            key: BindKey::Extra(wheel_dir),
            is_down: false,
            device: *device,
        }));
    }

    fn consumable_event(event: &native::native::WindowEvent) -> bool {
        // we basically only want input events to be consumable
        match event {
            native::native::WindowEvent::ActivationTokenDone { .. } => false,
            native::native::WindowEvent::Resized(_) => false,
            native::native::WindowEvent::Moved(_) => false,
            native::native::WindowEvent::CloseRequested => false,
            native::native::WindowEvent::Destroyed => false,
            native::native::WindowEvent::DroppedFile(_) => false,
            native::native::WindowEvent::HoveredFile(_) => false,
            native::native::WindowEvent::HoveredFileCancelled => false,
            native::native::WindowEvent::Focused(_) => false,
            native::native::WindowEvent::KeyboardInput { .. } => true,
            native::native::WindowEvent::ModifiersChanged(_) => true,
            native::native::WindowEvent::Ime(_) => true,
            native::native::WindowEvent::CursorMoved { .. } => true,
            native::native::WindowEvent::CursorEntered { .. } => true,
            native::native::WindowEvent::CursorLeft { .. } => true,
            native::native::WindowEvent::MouseWheel { .. } => true,
            native::native::WindowEvent::MouseInput { .. } => true,
            native::native::WindowEvent::TouchpadPressure { .. } => true,
            native::native::WindowEvent::AxisMotion { .. } => true,
            native::native::WindowEvent::Touch(_) => true,
            native::native::WindowEvent::ScaleFactorChanged { .. } => false,
            native::native::WindowEvent::ThemeChanged(_) => false,
            native::native::WindowEvent::Occluded(_) => false,
            native::native::WindowEvent::RedrawRequested => false,
            native::native::WindowEvent::PinchGesture { .. } => false,
            native::native::WindowEvent::PanGesture { .. } => false,
            native::native::WindowEvent::DoubleTapGesture { .. } => false,
            native::native::WindowEvent::RotationGesture { .. } => false,
        }
    }

    pub fn raw_event(&mut self, window: &Window, event: &native::native::WindowEvent) {
        if !Self::consumable_event(event) {
            return;
        }

        let _ = self.state.on_window_event(window, event);
    }
}
