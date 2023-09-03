use winit::{
    event::{DeviceId, MouseButton, WindowEvent},
    keyboard::KeyCode,
    window::Window,
};

pub mod binds;

pub trait InputEventHandler {
    fn key_down(&mut self, device: &DeviceId, key: KeyCode);
    fn key_up(&mut self, device: &DeviceId, key: KeyCode);
    fn mouse_down(&mut self, device: &DeviceId, x: f64, y: f64, btn: &MouseButton);
    fn mouse_up(&mut self, device: &DeviceId, x: f64, y: f64, btn: &MouseButton);
    fn mouse_move(&mut self, device: &DeviceId, x: f64, y: f64, xrel: f64, yrel: f64);

    /// returns true if the event has handled, indicating that it will be ignored for further logic
    fn raw_window_event<'a>(&mut self, window: &Window, event: &WindowEvent<'a>) -> bool;
}
