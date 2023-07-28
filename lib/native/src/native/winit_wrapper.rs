use anyhow::anyhow;
use base::{benchmark, system::SystemTimeInterface};
use hashlink::LinkedHashSet;
use winit::{
    event::Event,
    event_loop::EventLoop,
    window::{CursorGrabMode, Window},
};

use super::{FromNativeImpl, NativeCreateOptions, NativeImpl};

struct WindowMouse {
    last_user_mouse_mode_request: CursorGrabMode,
    last_mouse_mode: CursorGrabMode,

    cursor_main_pos: (f64, f64),
}

impl WindowMouse {
    fn mouse_grab_internal(
        &mut self,
        mode: CursorGrabMode,
        window: &Window,
        internal_events: Option<&mut LinkedHashSet<InternalEvent>>,
    ) -> bool {
        if self.last_mouse_mode != mode {
            match window.set_cursor_grab(mode) {
                Ok(_) => {
                    self.last_mouse_mode = mode;
                    if let CursorGrabMode::Confined = mode {
                        window.set_cursor_visible(true); // TODO: must be hidden for ingame, but visisble for GUI (or render own cursor)
                    } else {
                        window.set_cursor_visible(true);
                    }
                    true
                }
                Err(_) => {
                    if let Some(internal_events) = internal_events {
                        internal_events.insert(InternalEvent::MouseGrabWrong);
                    }
                    false
                }
            }
        } else {
            true
        }
    }

    fn try_set_mouse_grab(&mut self, window: &Window) -> anyhow::Result<()> {
        if self.last_mouse_mode != self.last_user_mouse_mode_request {
            if self.mouse_grab_internal(self.last_user_mouse_mode_request, window, None) {
                Ok(())
            } else {
                Err(anyhow!("mouse grab failed immediatelly."))
            }
        } else {
            Ok(())
        }
    }
}

pub(crate) struct WinitWindowWrapper {
    window: Window,

    mouse: WindowMouse,
    internal_events: LinkedHashSet<InternalEvent>,
}

impl NativeImpl for WinitWindowWrapper {
    fn mouse_grab(&mut self) {
        self.mouse.last_user_mouse_mode_request = CursorGrabMode::Confined;
        self.mouse.mouse_grab_internal(
            CursorGrabMode::Confined,
            &self.window,
            Some(&mut self.internal_events),
        );
    }
    fn borrow_window(&self) -> &Window {
        &self.window
    }
}

#[derive(Hash, PartialEq, Eq)]
enum InternalEvent {
    MouseGrabWrong,
}

pub(crate) struct WinitWrapper {
    event_loop: EventLoop<()>,
    pub(crate) window: WinitWindowWrapper,
}

impl WinitWrapper {
    pub fn new(native_options: NativeCreateOptions) -> Self {
        let event_loop = EventLoop::new();
        let window = benchmark!(
            native_options.do_bench,
            native_options.sys,
            "\tinitializing the window",
            || {
                winit::window::WindowBuilder::new()
                    .with_title(native_options.title)
                    .with_resizable(true)
                    .build(&event_loop)
                    .unwrap()
            }
        );
        Self {
            event_loop,
            window: WinitWindowWrapper {
                window,
                mouse: WindowMouse {
                    last_user_mouse_mode_request: CursorGrabMode::None,
                    last_mouse_mode: CursorGrabMode::None,
                    cursor_main_pos: Default::default(),
                },
                internal_events: Default::default(),
            },
        }
    }
}

impl WinitWrapper {
    pub(crate) fn run<F>(self, native_user: F) -> !
    where
        F: FromNativeImpl + 'static,
    {
        let mut window = self.window;
        let mut native_user_opt = Some(native_user);
        self.event_loop.run(move |event, _, control_flow| {
            control_flow.set_poll();

            let mut destroy = false;

            if let Some(native_user) = &mut native_user_opt {
                match &event {
                    Event::DeviceEvent { device_id, event } => match event {
                        winit::event::DeviceEvent::Added => todo!(),
                        winit::event::DeviceEvent::Removed => todo!(),
                        winit::event::DeviceEvent::MouseMotion { delta } => native_user.mouse_move(
                            device_id,
                            window.mouse.cursor_main_pos.0,
                            window.mouse.cursor_main_pos.1,
                            delta.0 as f64,
                            delta.1 as f64,
                        ),
                        winit::event::DeviceEvent::MouseWheel { delta: _ } => {}
                        winit::event::DeviceEvent::Motion { axis: _, value: _ } => {}
                        winit::event::DeviceEvent::Button { button: _, state } => match state {
                            winit::event::ElementState::Pressed => {}
                            winit::event::ElementState::Released => {}
                        },
                        winit::event::DeviceEvent::Key(key_input) => match key_input.state {
                            winit::event::ElementState::Pressed => {}
                            winit::event::ElementState::Released => {}
                        },
                        winit::event::DeviceEvent::Text { codepoint: _ } => todo!(),
                    },
                    Event::WindowEvent {
                        window_id: _,
                        event,
                    } => {
                        if !native_user.raw_window_event(&window.window, &event) {
                            match event {
                                winit::event::WindowEvent::Resized(new_size) => {
                                    native_user.resized(
                                        &mut window,
                                        new_size.width,
                                        new_size.height,
                                    );
                                }
                                winit::event::WindowEvent::Moved(_) => {} // TODO: important for canvas size
                                winit::event::WindowEvent::CloseRequested => {
                                    control_flow.set_exit()
                                }
                                winit::event::WindowEvent::Destroyed => {} // TODO: important for android
                                winit::event::WindowEvent::DroppedFile(_) => todo!(),
                                winit::event::WindowEvent::HoveredFile(_) => todo!(),
                                winit::event::WindowEvent::HoveredFileCancelled => todo!(),
                                winit::event::WindowEvent::Focused(has_focus) => {
                                    if !has_focus {
                                        window.mouse.mouse_grab_internal(
                                            CursorGrabMode::None,
                                            &window.window,
                                            Some(&mut window.internal_events),
                                        );
                                    } else {
                                        if let Err(_) =
                                            window.mouse.try_set_mouse_grab(&window.window)
                                        {
                                            window
                                                .internal_events
                                                .insert(InternalEvent::MouseGrabWrong);
                                        }
                                    }
                                } // TODO: also important for android
                                winit::event::WindowEvent::KeyboardInput {
                                    device_id,
                                    event,
                                    is_synthetic: _,
                                } => match event.state {
                                    winit::event::ElementState::Pressed => {
                                        native_user.key_down(device_id, event.physical_key)
                                    }
                                    winit::event::ElementState::Released => {
                                        native_user.key_up(device_id, event.physical_key)
                                    }
                                },
                                winit::event::WindowEvent::ModifiersChanged(_) => {}
                                winit::event::WindowEvent::Ime(_) => {}
                                winit::event::WindowEvent::CursorMoved {
                                    device_id,
                                    position,
                                    ..
                                } => {
                                    window.mouse.cursor_main_pos = (position.x, position.y);
                                    native_user
                                        .mouse_move(device_id, position.x, position.y, 0.0, 0.0)
                                }
                                winit::event::WindowEvent::CursorEntered { device_id: _ } => {}
                                winit::event::WindowEvent::CursorLeft { device_id: _ } => {}
                                winit::event::WindowEvent::MouseWheel {
                                    device_id: _,
                                    delta: _,
                                    phase: _,
                                    ..
                                } => {}
                                winit::event::WindowEvent::MouseInput {
                                    device_id,
                                    state,
                                    button,
                                    ..
                                } => match state {
                                    winit::event::ElementState::Pressed => native_user.mouse_down(
                                        device_id,
                                        window.mouse.cursor_main_pos.0,
                                        window.mouse.cursor_main_pos.1,
                                        button,
                                    ),
                                    winit::event::ElementState::Released => native_user.mouse_up(
                                        device_id,
                                        window.mouse.cursor_main_pos.0,
                                        window.mouse.cursor_main_pos.1,
                                        button,
                                    ),
                                },
                                winit::event::WindowEvent::TouchpadMagnify {
                                    device_id: _,
                                    delta: _,
                                    phase: _,
                                } => {}
                                winit::event::WindowEvent::SmartMagnify { device_id: _ } => {}
                                winit::event::WindowEvent::TouchpadRotate {
                                    device_id: _,
                                    delta: _,
                                    phase: _,
                                } => {}
                                winit::event::WindowEvent::TouchpadPressure {
                                    device_id: _,
                                    pressure: _,
                                    stage: _,
                                } => {}
                                winit::event::WindowEvent::AxisMotion {
                                    device_id: _,
                                    axis: _,
                                    value: _,
                                } => {}
                                winit::event::WindowEvent::Touch(_) => todo!(),
                                winit::event::WindowEvent::ScaleFactorChanged {
                                    scale_factor: _,
                                    new_inner_size: _,
                                } => {
                                    // TODO
                                }
                                winit::event::WindowEvent::ThemeChanged(_) => todo!(),
                                winit::event::WindowEvent::Occluded(_) => {}
                            }
                        }
                    }
                    Event::NewEvents(_) => {
                        // TODO: macos apparently needs to listen for the init event
                    }
                    Event::Resumed => {}
                    Event::UserEvent(_) => todo!(),
                    Event::Suspended => todo!(),
                    Event::MainEventsCleared => {
                        native_user.run(&mut window);

                        // check internal events
                        window.internal_events.retain_with_order(|ev| match ev {
                            InternalEvent::MouseGrabWrong => {
                                match window.mouse.try_set_mouse_grab(&window.window) {
                                    Ok(_) => false,
                                    Err(_) => true,
                                }
                            }
                        });
                    }
                    Event::RedrawRequested(_) => {}
                    Event::RedrawEventsCleared => {}
                    Event::LoopDestroyed => {
                        destroy = true;
                    }
                }
            }

            if destroy {
                native_user_opt.take().unwrap().destroy();
            }
        })
    }
}
