use winit::{
    event::{DeviceId, MouseButton, MouseScrollDelta, WindowEvent},
    keyboard::KeyCode,
    window::Window,
};

pub mod binds;

pub trait InputEventHandler {
    fn key_down(&mut self, window: &winit::window::Window, device: &DeviceId, key: KeyCode);
    fn key_up(&mut self, window: &winit::window::Window, device: &DeviceId, key: KeyCode);
    fn mouse_down(
        &mut self,
        window: &winit::window::Window,
        device: &DeviceId,
        x: f64,
        y: f64,
        btn: &MouseButton,
    );
    fn mouse_up(
        &mut self,
        window: &winit::window::Window,
        device: &DeviceId,
        x: f64,
        y: f64,
        btn: &MouseButton,
    );
    fn mouse_move(
        &mut self,
        window: &winit::window::Window,
        device: &DeviceId,
        x: f64,
        y: f64,
        xrel: f64,
        yrel: f64,
    );
    fn scroll(
        &mut self,
        window: &winit::window::Window,
        device: &DeviceId,
        x: f64,
        y: f64,
        delta: &MouseScrollDelta,
    );

    /// returns true if the event has handled, indicating that it will be ignored for further logic
    fn raw_window_event(&mut self, window: &Window, event: &WindowEvent) -> bool;
}
