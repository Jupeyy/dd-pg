use crate::{game::simulation_pipe::ClientPlayer, ui::ui::UI};

pub struct InputPipe<'a> {
    pub local_players: &'a mut [ClientPlayer; 4],
    pub ui: &'a mut UI,
}
use sdl2::keyboard::Scancode;

use native::input::InputEventHandler;

pub struct InputHandling<'a> {
    pub pipe: InputPipe<'a>,
}

impl<'a> InputEventHandler for InputHandling<'a> {
    fn key_down(&mut self, _device: u32, key: sdl2::keyboard::Scancode) {
        if key == Scancode::A {
            self.pipe.local_players[0].input.dir = -1;
        }
        if key == Scancode::D {
            self.pipe.local_players[0].input.dir = 1;
        }
        if key == Scancode::Space {
            self.pipe.local_players[0].input.jump = true;
        }
    }

    fn key_up(&mut self, _device: u32, key: sdl2::keyboard::Scancode) {
        if key == Scancode::A {
            self.pipe.local_players[0].input.dir = 0;
        }
        if key == Scancode::D {
            self.pipe.local_players[0].input.dir = 0;
        }
        if key == Scancode::Space {
            self.pipe.local_players[0].input.jump = false;
        }
    }

    fn mouse_down(&mut self, _device: u32, _x: i32, _y: i32, _btn: sdl2::mouse::MouseButton) {
        //
    }

    fn mouse_up(&mut self, _device: u32, _x: i32, _y: i32, _btn: sdl2::mouse::MouseButton) {
        //
    }

    fn mouse_move(&mut self, _device: u32, _x: i32, _y: i32, _xrel: i32, _yrel: i32) {
        //
    }

    fn raw_event(&mut self, window: &sdl2::video::Window, event: &sdl2::event::Event) -> bool {
        if self.pipe.ui.ui_state.is_ui_open && !event.is_window() && event.get_window_id().is_some()
        {
            self.pipe
                .ui
                .ui_state
                .sdl2_state
                .sdl2_input_to_egui(&window, event);
            return true;
        }
        return false;
    }
}
