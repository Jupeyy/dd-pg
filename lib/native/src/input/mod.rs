pub mod sdl_to_egui;

use sdl2::{keyboard::Scancode, mouse::MouseButton, *};

use super::native::Native;

pub trait InputEventHandler {
    fn key_down(&mut self, device: u32, key: Scancode);
    fn key_up(&mut self, device: u32, key: Scancode);
    fn mouse_down(&mut self, device: u32, x: i32, y: i32, btn: MouseButton);
    fn mouse_up(&mut self, device: u32, x: i32, y: i32, btn: MouseButton);
    fn mouse_move(&mut self, device: u32, x: i32, y: i32, xrel: i32, yrel: i32);

    // returns true if the event has handled, indicating that it will be ignored for further logic
    fn raw_event(&mut self, window: &sdl2::video::Window, event: &sdl2::event::Event) -> bool;
}

pub trait WindowEventHandler {
    fn borrow_window(&self) -> &sdl2::video::Window;
    fn resized(&mut self, new_width: u32, new_height: u32);
}

pub struct Input {
    sdl2: Sdl,
}

impl Input {
    pub fn new(native: Native) -> Input {
        let inp = Input { sdl2: native.sdl2 };
        inp.init();
        inp
    }

    fn init(&self) {
        self.sdl2.mouse().set_relative_mouse_mode(true);
    }

    pub fn run(
        &self,
        inp_handler: &mut dyn InputEventHandler,
        window_handler: &mut dyn WindowEventHandler,
    ) -> bool {
        let mut event_queue = self.sdl2.event_pump().unwrap();
        for event in event_queue.poll_iter() {
            if !inp_handler.raw_event(window_handler.borrow_window(), &event) {
                match event {
                    event::Event::KeyDown { scancode, .. } => {
                        if let Some(scancode_key) = scancode {
                            inp_handler.key_down(0, scancode_key);
                        }
                    }
                    event::Event::KeyUp { scancode, .. } => {
                        if let Some(scancode_key) = scancode {
                            inp_handler.key_up(0, scancode_key);
                        }
                    }
                    event::Event::MouseButtonDown {
                        mouse_btn,
                        x,
                        y,
                        which,
                        ..
                    } => {
                        inp_handler.mouse_move(which, x, y, 0, 0);
                        inp_handler.mouse_down(which, x, y, mouse_btn);
                    }
                    event::Event::MouseButtonUp {
                        mouse_btn,
                        x,
                        y,
                        which,
                        ..
                    } => {
                        inp_handler.mouse_move(which, x, y, 0, 0);
                        inp_handler.mouse_up(which, x, y, mouse_btn);
                    }
                    event::Event::MouseMotion {
                        x,
                        y,
                        xrel,
                        yrel,
                        which,
                        ..
                    } => {
                        inp_handler.mouse_move(which, x, y, xrel, yrel);
                    }
                    event::Event::Window {
                        timestamp: _,
                        window_id: _,
                        win_event,
                    } => match win_event {
                        event::WindowEvent::SizeChanged(x, y) => {
                            window_handler.resized(x as u32, y as u32)
                        }
                        _ => {}
                    },
                    event::Event::Quit { .. } => return false,
                    _e => {}
                }
            }
        }
        return true;
    }
}
