use super::graphics::graphics::Graphics;

pub struct WindowEventPipe<'a> {
    pub graphics: &'a mut Graphics,
}

pub struct WindowHandling<'a> {
    pub pipe: WindowEventPipe<'a>,
}
