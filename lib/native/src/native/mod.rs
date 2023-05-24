use sdl2::*;

#[derive(Clone)]
pub struct Native {
    pub sdl2: Sdl,
}
impl Native {
    pub fn new() -> Native {
        Native {
            sdl2: sdl2::init().unwrap(),
        }
    }
}
