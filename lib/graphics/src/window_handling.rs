use native::input::WindowEventHandler;

use super::graphics::Graphics;

pub struct WindowEventPipe<'a> {
    pub graphics: &'a mut Graphics,
}

pub struct WindowHandling<'a> {
    pub pipe: WindowEventPipe<'a>,
}

impl<'a> WindowEventHandler for WindowHandling<'a> {
    fn resized(&mut self, new_width: u32, new_height: u32) {
        self.pipe.graphics.resized(new_width, new_height);
    }

    fn borrow_window(&self) -> &sdl2::video::Window {
        self.pipe.graphics.borrow_window()
    }
}
