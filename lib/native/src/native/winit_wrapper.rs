use std::cell::Cell;

use anyhow::anyhow;
use base::benchmark::Benchmark;
use hashlink::LinkedHashSet;
use raw_window_handle::HasDisplayHandle;
use winit::{
    event::Event,
    event_loop::EventLoop,
    window::{CursorGrabMode, Window},
};

use super::{FromNativeImpl, FromNativeLoadingImpl, NativeCreateOptions, NativeImpl};

struct WindowMouse {
    last_user_mouse_mode_request: CursorGrabMode,
    last_user_mouse_cursor_mode: bool,
    last_mouse_mode: CursorGrabMode,
    last_mouse_cursor_mode: bool,

    cursor_main_pos: (f64, f64),

    dbg_mode: bool,
}

impl WindowMouse {
    fn toggle_cursor_internal(&mut self, show: bool, window: &Window) -> bool {
        if self.last_mouse_cursor_mode != show && !self.dbg_mode {
            self.last_mouse_cursor_mode = show;
            window.set_cursor_visible(show);
            true
        } else {
            true
        }
    }
    fn mouse_grab_internal(
        &mut self,
        mode: CursorGrabMode,
        window: &Window,
        internal_events: Option<&mut LinkedHashSet<InternalEvent>>,
    ) -> bool {
        if self.last_mouse_mode != mode && !self.dbg_mode {
            match window.set_cursor_grab(mode) {
                Ok(_) => {
                    self.last_mouse_mode = mode;
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

    destroy: Cell<bool>,
    start_arguments: Vec<String>,
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
    fn toggle_cursor(&mut self, show: bool) {
        self.mouse.last_user_mouse_cursor_mode = show;
        self.mouse.toggle_cursor_internal(show, &self.window);
    }
    fn borrow_window(&self) -> &Window {
        &self.window
    }
    fn quit(&self) {
        self.destroy.set(true);
    }
    fn start_arguments(&self) -> &Vec<String> {
        &self.start_arguments
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
    pub fn new<F, L>(native_options: NativeCreateOptions, loading: &mut L) -> anyhow::Result<Self>
    where
        L: Sized,
        F: FromNativeLoadingImpl<L>,
    {
        let benchmark = Benchmark::new(native_options.do_bench);
        let event_loop = EventLoop::new().unwrap();
        benchmark.bench("initializing the event loop");
        F::load_with_display_handle(loading, event_loop.display_handle()?.as_raw())?;

        let window = winit::window::WindowBuilder::new()
            .with_title(native_options.title)
            .with_resizable(true)
            .build(&event_loop)
            .unwrap();
        benchmark.bench("initializing the window");

        Ok(Self {
            event_loop,
            window: WinitWindowWrapper {
                window,
                mouse: WindowMouse {
                    last_user_mouse_mode_request: CursorGrabMode::None,
                    last_user_mouse_cursor_mode: true,
                    last_mouse_mode: CursorGrabMode::None,
                    last_mouse_cursor_mode: true,
                    cursor_main_pos: Default::default(),

                    dbg_mode: native_options.dbg_input,
                },
                internal_events: Default::default(),
                destroy: Default::default(),
                start_arguments: native_options.start_arguments,
            },
        })
    }
}

impl WinitWrapper {
    pub(crate) fn run<F>(self, native_user: F) -> anyhow::Result<()>
    where
        F: FromNativeImpl + 'static,
    {
        let mut window = self.window;
        window.window.request_redraw();
        let mut native_user_opt = Some(native_user);
        Ok(self.event_loop.run(move |event, event_loop| {
            event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);

            if let Some(native_user) = &mut native_user_opt {
                match &event {
                    Event::DeviceEvent { device_id, event } => match event {
                        winit::event::DeviceEvent::Added => {
                            // TODO:
                        }
                        winit::event::DeviceEvent::Removed => {
                            // TODO:
                        }
                        winit::event::DeviceEvent::MouseMotion {
                            delta: (delta_x, delta_y),
                        } => native_user.mouse_move(
                            &window.window,
                            device_id,
                            window.mouse.cursor_main_pos.0,
                            window.mouse.cursor_main_pos.1,
                            *delta_x as f64,
                            *delta_y as f64,
                        ),
                        winit::event::DeviceEvent::MouseWheel { .. } => {
                            /* TODO: the other mouse wheel event sends the opposite native_user.scroll(
                                device_id,
                                window.mouse.cursor_main_pos.0,
                                window.mouse.cursor_main_pos.1,
                                delta,
                            );*/
                        }
                        winit::event::DeviceEvent::Motion { axis: _, value: _ } => {}
                        winit::event::DeviceEvent::Button { button: _, state } => match state {
                            winit::event::ElementState::Pressed => {}
                            winit::event::ElementState::Released => {}
                        },
                        winit::event::DeviceEvent::Key(key_input) => match key_input.state {
                            winit::event::ElementState::Pressed => {}
                            winit::event::ElementState::Released => {}
                        },
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
                                    event_loop.exit();
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
                                } => {
                                    if !event.repeat {
                                        match event.state {
                                            winit::event::ElementState::Pressed => native_user
                                                .key_down(
                                                    &window.window,
                                                    device_id,
                                                    event.physical_key,
                                                ),
                                            winit::event::ElementState::Released => native_user
                                                .key_up(
                                                    &window.window,
                                                    device_id,
                                                    event.physical_key,
                                                ),
                                        }
                                    }
                                }
                                winit::event::WindowEvent::ModifiersChanged(_) => {}
                                winit::event::WindowEvent::Ime(_) => {}
                                winit::event::WindowEvent::CursorMoved {
                                    device_id,
                                    position,
                                    ..
                                } => {
                                    window.mouse.cursor_main_pos = (position.x, position.y);
                                    native_user.mouse_move(
                                        &window.window,
                                        device_id,
                                        position.x,
                                        position.y,
                                        0.0,
                                        0.0,
                                    )
                                }
                                winit::event::WindowEvent::CursorEntered { device_id: _ } => {}
                                winit::event::WindowEvent::CursorLeft { device_id: _ } => {}
                                winit::event::WindowEvent::MouseWheel {
                                    device_id,
                                    delta,
                                    phase: _,
                                    ..
                                } => native_user.scroll(
                                    &window.window,
                                    device_id,
                                    window.mouse.cursor_main_pos.0,
                                    window.mouse.cursor_main_pos.1,
                                    delta,
                                ),
                                winit::event::WindowEvent::MouseInput {
                                    device_id,
                                    state,
                                    button,
                                    ..
                                } => match state {
                                    winit::event::ElementState::Pressed => native_user.mouse_down(
                                        &window.window,
                                        device_id,
                                        window.mouse.cursor_main_pos.0,
                                        window.mouse.cursor_main_pos.1,
                                        button,
                                    ),
                                    winit::event::ElementState::Released => native_user.mouse_up(
                                        &window.window,
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
                                    inner_size_writer: _,
                                } => {
                                    // TODO
                                    let inner_size = window.borrow_window().inner_size();
                                    native_user.resized(
                                        &mut window,
                                        inner_size.width,
                                        inner_size.height,
                                    );
                                }
                                winit::event::WindowEvent::ThemeChanged(_) => todo!(),
                                winit::event::WindowEvent::Occluded(_) => {}
                                winit::event::WindowEvent::ActivationTokenDone {
                                    serial: _,
                                    token: _,
                                } => todo!(),
                                winit::event::WindowEvent::RedrawRequested => {
                                    window.window.request_redraw();
                                }
                            }
                        }
                    }
                    Event::NewEvents(_) => {
                        // TODO: macos apparently needs to listen for the init event
                    }
                    Event::Resumed => {}
                    Event::UserEvent(_) => todo!(),
                    Event::Suspended => todo!(),
                    Event::MemoryWarning => {
                        // don't handle for now
                    }
                    Event::LoopExiting => {
                        window.destroy.set(true);
                    }
                    Event::AboutToWait => {
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
                }
            }

            if window.destroy.get() {
                event_loop.exit();
                if let Some(native_user) = native_user_opt.take() {
                    native_user.destroy();
                }
            }
        })?)
    }
}
