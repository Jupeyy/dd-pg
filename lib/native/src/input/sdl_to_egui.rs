/**
 * this is mostly from:
 *  https://github.com/kaphula/egui-sdl2-event/blob/f0d6597f3fc86c28db8024e2a7080a2454fb9b1e/src/lib.rs
 * just modified to fit our needs better
 */
use egui::{Key, Modifiers, PointerButton, Pos2, RawInput};

use sdl2::keyboard::Keycode;
use sdl2::keyboard::Mod;
use sdl2::mouse::{Cursor, MouseButton, SystemCursor};
use sdl2::video::Window;

pub struct FusedCursor {
    pub cursor: sdl2::mouse::Cursor,
    pub icon: sdl2::mouse::SystemCursor,
}

impl FusedCursor {
    pub fn new() -> Self {
        Self {
            cursor: sdl2::mouse::Cursor::from_system(sdl2::mouse::SystemCursor::Arrow).unwrap(),
            icon: sdl2::mouse::SystemCursor::Arrow,
        }
    }
}

impl Default for FusedCursor {
    fn default() -> Self {
        Self::new()
    }
}

pub fn translate_virtual_key_code(key: sdl2::keyboard::Keycode) -> Option<egui::Key> {
    use Keycode::*;

    Some(match key {
        Left => Key::ArrowLeft,
        Up => Key::ArrowUp,
        Right => Key::ArrowRight,
        Down => Key::ArrowDown,

        Escape => Key::Escape,
        Tab => Key::Tab,
        Backspace => Key::Backspace,
        Space => Key::Space,
        Return => Key::Enter,

        Insert => Key::Insert,
        Home => Key::Home,
        Delete => Key::Delete,
        End => Key::End,
        PageDown => Key::PageDown,
        PageUp => Key::PageUp,

        Kp0 | Num0 => Key::Num0,
        Kp1 | Num1 => Key::Num1,
        Kp2 | Num2 => Key::Num2,
        Kp3 | Num3 => Key::Num3,
        Kp4 | Num4 => Key::Num4,
        Kp5 | Num5 => Key::Num5,
        Kp6 | Num6 => Key::Num6,
        Kp7 | Num7 => Key::Num7,
        Kp8 | Num8 => Key::Num8,
        Kp9 | Num9 => Key::Num9,

        A => Key::A,
        B => Key::B,
        C => Key::C,
        D => Key::D,
        E => Key::E,
        F => Key::F,
        G => Key::G,
        H => Key::H,
        I => Key::I,
        J => Key::J,
        K => Key::K,
        L => Key::L,
        M => Key::M,
        N => Key::N,
        O => Key::O,
        P => Key::P,
        Q => Key::Q,
        R => Key::R,
        S => Key::S,
        T => Key::T,
        U => Key::U,
        V => Key::V,
        W => Key::W,
        X => Key::X,
        Y => Key::Y,
        Z => Key::Z,

        _ => {
            return None;
        }
    })
}

pub struct EguiSDL2State {
    pub raw_input: RawInput,
    pub modifiers: Modifiers,
    pub cur_zoom_level: f32,
    // pub fused_cursor: FusedCursor,
}

impl EguiSDL2State {
    pub fn sdl2_input_to_egui(&mut self, window: &sdl2::video::Window, event: &sdl2::event::Event) {
        fn sdl_button_to_egui(btn: &MouseButton) -> Option<PointerButton> {
            match btn {
                MouseButton::Left => Some(egui::PointerButton::Primary),
                MouseButton::Middle => Some(egui::PointerButton::Middle),
                MouseButton::Right => Some(egui::PointerButton::Secondary),
                _ => None,
            }
        }

        use sdl2::event::Event::*;
        if event.get_window_id() != Some(window.id()) {
            return;
        }
        match event {
            MouseButtonDown {
                mouse_btn, x, y, ..
            } => {
                if let Some(pressed) = sdl_button_to_egui(mouse_btn) {
                    self.raw_input.events.push(egui::Event::PointerButton {
                        pos: Pos2 {
                            x: *x as f32 / self.cur_zoom_level,
                            y: *y as f32 / self.cur_zoom_level,
                        },
                        button: pressed,
                        pressed: true,
                        modifiers: self.modifiers,
                    });
                }
            }
            MouseButtonUp {
                mouse_btn, x, y, ..
            } => {
                if let Some(released) = sdl_button_to_egui(mouse_btn) {
                    self.raw_input.events.push(egui::Event::PointerButton {
                        pos: Pos2 {
                            x: *x as f32 / self.cur_zoom_level,
                            y: *y as f32 / self.cur_zoom_level,
                        },
                        button: released,
                        pressed: false,
                        modifiers: self.modifiers,
                    });
                }
            }

            MouseMotion { x, y, .. } => {
                self.raw_input.events.push(egui::Event::PointerMoved(Pos2 {
                    x: *x as f32 / self.cur_zoom_level,
                    y: *y as f32 / self.cur_zoom_level,
                }));
            }

            KeyUp {
                keycode,
                keymod,
                repeat,
                ..
            } => {
                let key_code = match keycode {
                    Some(key_code) => key_code,
                    _ => return,
                };
                let key = match translate_virtual_key_code(*key_code) {
                    Some(key) => key,
                    _ => return,
                };
                self.modifiers = Modifiers {
                    alt: (*keymod & Mod::LALTMOD == Mod::LALTMOD)
                        || (*keymod & Mod::RALTMOD == Mod::RALTMOD),
                    ctrl: (*keymod & Mod::LCTRLMOD == Mod::LCTRLMOD)
                        || (*keymod & Mod::RCTRLMOD == Mod::RCTRLMOD),
                    shift: (*keymod & Mod::LSHIFTMOD == Mod::LSHIFTMOD)
                        || (*keymod & Mod::RSHIFTMOD == Mod::RSHIFTMOD),
                    mac_cmd: *keymod & Mod::LGUIMOD == Mod::LGUIMOD,

                    //TOD: Test on both windows and mac
                    command: (*keymod & Mod::LCTRLMOD == Mod::LCTRLMOD)
                        || (*keymod & Mod::LGUIMOD == Mod::LGUIMOD),
                };

                self.raw_input.events.push(egui::Event::Key {
                    key,
                    pressed: false,
                    modifiers: self.modifiers,
                    repeat: *repeat,
                });
            }

            KeyDown {
                keycode,
                keymod,
                repeat,
                ..
            } => {
                let key_code = match keycode {
                    Some(key_code) => key_code,
                    _ => return,
                };

                let key = match translate_virtual_key_code(*key_code) {
                    Some(key) => key,
                    _ => return,
                };
                self.modifiers = Modifiers {
                    alt: (*keymod & Mod::LALTMOD == Mod::LALTMOD)
                        || (*keymod & Mod::RALTMOD == Mod::RALTMOD),
                    ctrl: (*keymod & Mod::LCTRLMOD == Mod::LCTRLMOD)
                        || (*keymod & Mod::RCTRLMOD == Mod::RCTRLMOD),
                    shift: (*keymod & Mod::LSHIFTMOD == Mod::LSHIFTMOD)
                        || (*keymod & Mod::RSHIFTMOD == Mod::RSHIFTMOD),
                    mac_cmd: *keymod & Mod::LGUIMOD == Mod::LGUIMOD,

                    //TOD: Test on both windows and mac
                    command: (*keymod & Mod::LCTRLMOD == Mod::LCTRLMOD)
                        || (*keymod & Mod::LGUIMOD == Mod::LGUIMOD),
                };

                self.raw_input.events.push(egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers: self.modifiers,
                    repeat: *repeat,
                });

                if self.modifiers.command && key == Key::C {
                    self.raw_input.events.push(egui::Event::Copy);
                } else if self.modifiers.command && key == Key::X {
                    self.raw_input.events.push(egui::Event::Cut);
                } else if self.modifiers.command && key == Key::V {
                    if let Ok(contents) = window.subsystem().clipboard().clipboard_text() {
                        self.raw_input.events.push(egui::Event::Text(contents));
                    }
                }
            }

            TextInput { text, .. } => {
                self.raw_input.events.push(egui::Event::Text(text.clone()));
            }
            MouseWheel { x, y, .. } => {
                let delta = egui::vec2(*x as f32 * 8.0, *y as f32 * 8.0);
                let sdl = window.subsystem().sdl();
                // zoom:
                if sdl.keyboard().mod_state() & Mod::LCTRLMOD == Mod::LCTRLMOD
                    || sdl.keyboard().mod_state() & Mod::RCTRLMOD == Mod::RCTRLMOD
                {
                    let zoom_delta = (delta.y / 125.0).exp();
                    self.raw_input.events.push(egui::Event::Zoom(zoom_delta));
                }
                // horizontal scroll:
                else if sdl.keyboard().mod_state() & Mod::LSHIFTMOD == Mod::LSHIFTMOD
                    || sdl.keyboard().mod_state() & Mod::RSHIFTMOD == Mod::RSHIFTMOD
                {
                    self.raw_input
                        .events
                        .push(egui::Event::Scroll(egui::vec2(delta.x + delta.y, 0.0)));
                    // regular scroll:
                } else {
                    self.raw_input
                        .events
                        .push(egui::Event::Scroll(egui::vec2(delta.x, delta.y)));
                }
            }
            _ => {}
        }
    }

    pub fn new(cur_zoom: f32) -> Self {
        let raw_input = RawInput {
            ..RawInput::default()
        };
        let modifiers = Modifiers::default();
        EguiSDL2State {
            raw_input,
            modifiers,
            //fused_cursor: FusedCursor::new(),
            cur_zoom_level: cur_zoom,
        }
    }

    pub fn process_output(&mut self, window: &Window, egui_output: &egui::PlatformOutput) {
        if !egui_output.copied_text.is_empty() {
            let copied_text = egui_output.copied_text.clone();
            {
                let result = window
                    .subsystem()
                    .clipboard()
                    .set_clipboard_text(&copied_text);
                if result.is_err() {
                    dbg!("Unable to set clipboard content to SDL clipboard.");
                }
            }
        }
        //EguiSDL2State::translate_cursor(&mut self.fused_cursor, egui_output.cursor_icon);
    }

    fn _translate_cursor(fused: &mut FusedCursor, cursor_icon: egui::CursorIcon) {
        let tmp_icon = match cursor_icon {
            egui::CursorIcon::Crosshair => SystemCursor::Crosshair,
            egui::CursorIcon::Default => SystemCursor::Arrow,
            egui::CursorIcon::Grab => SystemCursor::Hand,
            egui::CursorIcon::Grabbing => SystemCursor::SizeAll,
            egui::CursorIcon::Move => SystemCursor::SizeAll,
            egui::CursorIcon::PointingHand => SystemCursor::Hand,
            egui::CursorIcon::ResizeHorizontal => SystemCursor::SizeWE,
            egui::CursorIcon::ResizeNeSw => SystemCursor::SizeNESW,
            egui::CursorIcon::ResizeNwSe => SystemCursor::SizeNWSE,
            egui::CursorIcon::ResizeVertical => SystemCursor::SizeNS,
            egui::CursorIcon::Text => SystemCursor::IBeam,
            egui::CursorIcon::NotAllowed | egui::CursorIcon::NoDrop => SystemCursor::No,
            egui::CursorIcon::Wait => SystemCursor::Wait,
            //There doesn't seem to be a suitable SDL equivalent...
            _ => SystemCursor::Arrow,
        };

        if tmp_icon != fused.icon {
            fused.cursor = Cursor::from_system(tmp_icon).unwrap();
            fused.icon = tmp_icon;
            fused.cursor.set();
        }
    }
}
