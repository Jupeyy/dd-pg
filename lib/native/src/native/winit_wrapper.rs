use std::cell::Cell;

use anyhow::anyhow;
use base::benchmark::Benchmark;
use hashlink::LinkedHashSet;
use raw_window_handle::HasDisplayHandle;
use winit::{
    dpi::{PhysicalSize, Size},
    event::Event,
    event_loop::EventLoop,
    monitor::{MonitorHandle, VideoMode},
    window::{CursorGrabMode, Fullscreen, Window},
};

use crate::native::app::{MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH};

use super::{
    app::NativeApp, FromNativeImpl, FromNativeLoadingImpl, NativeCreateOptions, NativeImpl,
    NativeWindowMonitorDetails, NativeWindowOptions,
};

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

impl WinitWindowWrapper {
    fn find_monitor_and_video_mode(
        available_monitors: impl Fn() -> Box<dyn Iterator<Item = MonitorHandle>>,
        primary_monitor: Option<MonitorHandle>,
        wnd: &NativeWindowOptions,
    ) -> anyhow::Result<(MonitorHandle, Option<VideoMode>)> {
        let monitor = available_monitors().find(|monitor| {
            monitor
                .name()
                .as_ref()
                .map(|name| (name.as_str(), monitor.size()))
                == wnd
                    .monitor
                    .as_ref()
                    .map(|monitor| (monitor.name.as_str(), monitor.size))
        });

        let video_mode = if let Some(monitor) = &monitor {
            monitor
                .video_modes()
                .find(|video_mode| {
                    video_mode.refresh_rate_millihertz() == wnd.refresh_rate_milli_hertz
                        && video_mode.size().width == wnd.width
                        && video_mode.size().height == wnd.height
                })
                .or_else(|| {
                    // try to find ignoring the refresh rate
                    monitor.video_modes().find(|video_mode| {
                        video_mode.size().width == wnd.width
                            && video_mode.size().height == wnd.height
                    })
                })
        } else {
            None
        };

        let Some(monitor) = monitor
            .or(primary_monitor)
            .or_else(|| available_monitors().next())
        else {
            return Err(anyhow!("no monitor found."));
        };
        Ok((monitor, video_mode))
    }

    fn fullscreen_mode(
        monitor: MonitorHandle,
        video_mode: Option<VideoMode>,
        wnd: &NativeWindowOptions,
    ) -> Option<Fullscreen> {
        if !wnd.fullscreen && wnd.maximized && !wnd.decorated {
            Some(winit::window::Fullscreen::Borderless(Some(monitor)))
        } else if wnd.fullscreen {
            if let Some(video_mode) = video_mode.or_else(|| {
                monitor.video_modes().max_by(|v1, v2| {
                    let size1 = v1.size();
                    let size2 = v2.size();
                    let mut cmp = size1.width.cmp(&size2.width);
                    if matches!(cmp, std::cmp::Ordering::Equal) {
                        cmp = size1.height.cmp(&size2.height);
                        if matches!(cmp, std::cmp::Ordering::Equal) {
                            cmp = v1
                                .refresh_rate_millihertz()
                                .cmp(&v2.refresh_rate_millihertz());
                        };
                    }
                    cmp
                })
            }) {
                Some(winit::window::Fullscreen::Exclusive(video_mode))
            } else {
                Some(winit::window::Fullscreen::Borderless(Some(monitor)))
            }
        } else {
            None
        }
    }
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
    fn set_window_config(&mut self, wnd: NativeWindowOptions) -> anyhow::Result<()> {
        let (monitor, video_mode) = WinitWindowWrapper::find_monitor_and_video_mode(
            || Box::new(self.window.available_monitors()),
            self.window.primary_monitor(),
            &wnd,
        )?;
        let fullscreen_mode = Self::fullscreen_mode(monitor, video_mode, &wnd);
        if fullscreen_mode.is_none() {
            let _ = self
                .window
                .request_inner_size(Size::Physical(winit::dpi::PhysicalSize {
                    width: wnd.width,
                    height: wnd.height,
                }));

            self.window.set_maximized(wnd.maximized);
            self.window.set_decorations(wnd.decorated);
        }
        self.window.set_fullscreen(fullscreen_mode);

        Ok(())
    }
    fn borrow_window(&self) -> &Window {
        &self.window
    }
    fn monitors(&self) -> Vec<MonitorHandle> {
        self.window.available_monitors().collect()
    }
    fn window_options(&self) -> NativeWindowOptions {
        let (refresh_rate_milli_hertz, monitor_name) = self
            .window
            .current_monitor()
            .map(|monitor| {
                (
                    monitor.refresh_rate_millihertz().unwrap_or_default(),
                    monitor.name().map(|name| {
                        let size = monitor.size();
                        NativeWindowMonitorDetails { name, size }
                    }),
                )
            })
            .unwrap_or_default();

        NativeWindowOptions {
            fullscreen: self
                .window
                .fullscreen()
                .is_some_and(|f| matches!(f, Fullscreen::Exclusive(_))),
            decorated: self.window.is_decorated()
                && !self
                    .window
                    .fullscreen()
                    .is_some_and(|f| matches!(f, Fullscreen::Borderless(_))),
            maximized: self.window.is_maximized()
                || self
                    .window
                    .fullscreen()
                    .is_some_and(|f| matches!(f, Fullscreen::Borderless(_))),
            width: self.window.inner_size().width.max(MIN_WINDOW_WIDTH),
            height: self.window.inner_size().height.max(MIN_WINDOW_HEIGHT),
            refresh_rate_milli_hertz,
            monitor: monitor_name,
        }
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
    event_loop: EventLoop<NativeApp>,
    pub(crate) window: WinitWindowWrapper,
}

impl WinitWrapper {
    pub fn new<F, L>(
        native_options: NativeCreateOptions,
        app: NativeApp,
        loading: &mut L,
    ) -> anyhow::Result<Self>
    where
        L: Sized,
        F: FromNativeLoadingImpl<L>,
    {
        let benchmark = Benchmark::new(native_options.do_bench);
        #[cfg_attr(not(target_os = "android"), allow(clippy::let_unit_value))]
        let _ = app;
        #[cfg(not(target_os = "android"))]
        let event_loop = EventLoop::new()?;
        #[cfg(target_os = "android")]
        use winit::platform::android::EventLoopBuilderExtAndroid;
        #[cfg(target_os = "android")]
        let event_loop = winit::event_loop::EventLoopBuilder::with_user_event()
            .with_android_app(app)
            .build()?;
        benchmark.bench("initializing the event loop");
        F::load_with_display_handle(
            loading,
            event_loop
                .display_handle()
                .map_err(|err| anyhow!("failed to get display handle for load operation: {err}"))?
                .as_raw(),
        )?;

        let (monitor, video_mode) = WinitWindowWrapper::find_monitor_and_video_mode(
            || Box::new(event_loop.available_monitors()),
            event_loop.primary_monitor(),
            &native_options.window,
        )?;

        let fullscreen_mode =
            WinitWindowWrapper::fullscreen_mode(monitor, video_mode, &native_options.window);

        let mut window_builder = winit::window::WindowBuilder::new()
            .with_title(native_options.title)
            .with_resizable(true)
            .with_active(true)
            .with_min_inner_size(Size::Physical(winit::dpi::PhysicalSize {
                width: MIN_WINDOW_WIDTH,
                height: MIN_WINDOW_HEIGHT,
            }))
            .with_theme(Some(winit::window::Theme::Dark));
        if fullscreen_mode.is_none() {
            window_builder = window_builder
                .with_inner_size(Size::Physical(winit::dpi::PhysicalSize {
                    width: native_options.window.width,
                    height: native_options.window.height,
                }))
                .with_maximized(native_options.window.maximized)
                .with_decorations(native_options.window.decorated);
        }
        window_builder = window_builder.with_fullscreen(fullscreen_mode);

        let window = window_builder.build(&event_loop).unwrap();
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

    #[cfg(target_os = "linux")]
    fn should_check_surface_create<T>(event_loop: &EventLoop<T>) -> bool {
        use winit::platform::wayland::EventLoopWindowTargetExtWayland;
        event_loop.is_wayland()
    }
    #[cfg(target_os = "android")]
    fn should_check_surface_create<T>(_: &EventLoop<T>) -> bool {
        true
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    fn should_check_surface_create<T>(_: &EventLoop<T>) -> bool {
        false
    }

    pub(crate) fn run<F, L>(self, native_user_loading: L) -> anyhow::Result<()>
    where
        F: FromNativeImpl + FromNativeLoadingImpl<L> + 'static,
    {
        let respect_surface_create = Self::should_check_surface_create(&self.event_loop);

        let mut window = self.window;
        window.window.request_redraw();

        enum NativeUser<F> {
            Some(F),
            Wait,
            None,
        }

        impl<F> NativeUser<F> {
            fn take(&mut self) -> Self {
                std::mem::replace(self, Self::None)
            }
        }

        let mut native_user_loading = Some(native_user_loading);
        let mut native_user_opt = if !respect_surface_create {
            NativeUser::Some(F::new(native_user_loading.take().unwrap(), &mut window)?)
        } else {
            NativeUser::Wait
        };

        Ok(self.event_loop.run(move |event, event_loop| {
            event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);

            match &mut native_user_opt {
                NativeUser::Some(native_user) => {
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
                                *delta_x,
                                *delta_y,
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
                            if !native_user.raw_window_event(&window.window, event) {
                                match event {
                                    winit::event::WindowEvent::Resized(new_size) => {
                                        native_user.resized(
                                            &mut window,
                                            new_size.width,
                                            new_size.height,
                                        );
                                        native_user.window_options_changed(window.window_options());
                                    }
                                    winit::event::WindowEvent::Moved(_) => {}
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
                                        } else if window
                                            .mouse
                                            .try_set_mouse_grab(&window.window)
                                            .is_err()
                                        {
                                            window
                                                .internal_events
                                                .insert(InternalEvent::MouseGrabWrong);
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
                                        winit::event::ElementState::Pressed => native_user
                                            .mouse_down(
                                                &window.window,
                                                device_id,
                                                window.mouse.cursor_main_pos.0,
                                                window.mouse.cursor_main_pos.1,
                                                button,
                                            ),
                                        winit::event::ElementState::Released => native_user
                                            .mouse_up(
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
                                    winit::event::WindowEvent::Touch(touch) => {
                                        native_user.mouse_down(
                                            &window.window,
                                            &touch.device_id,
                                            touch.location.x,
                                            touch.location.y,
                                            &winit::event::MouseButton::Left,
                                        );
                                        native_user.mouse_up(
                                            &window.window,
                                            &touch.device_id,
                                            touch.location.x,
                                            touch.location.y,
                                            &winit::event::MouseButton::Left,
                                        );
                                    }
                                    winit::event::WindowEvent::ScaleFactorChanged {
                                        scale_factor: _,
                                        inner_size_writer: _,
                                    } => {
                                        // TODO
                                        let inner_size = window.borrow_window().inner_size().clamp(
                                            PhysicalSize {
                                                width: MIN_WINDOW_WIDTH,
                                                height: MIN_WINDOW_HEIGHT,
                                            },
                                            PhysicalSize {
                                                width: u32::MAX,
                                                height: u32::MAX,
                                            },
                                        );
                                        native_user.resized(
                                            &mut window,
                                            inner_size.width,
                                            inner_size.height,
                                        );
                                        native_user.window_options_changed(window.window_options());
                                    }
                                    winit::event::WindowEvent::ThemeChanged(_) => {
                                        // not really interesting
                                    }
                                    winit::event::WindowEvent::Occluded(_) => {}
                                    winit::event::WindowEvent::ActivationTokenDone {
                                        serial: _,
                                        token: _,
                                    } => {
                                        // no idea what this is
                                    }
                                    winit::event::WindowEvent::RedrawRequested => {
                                        window.window.request_redraw();
                                    }
                                }
                            }
                        }
                        Event::NewEvents(_) => {
                            // TODO: macos apparently needs to listen for the init event
                        }
                        Event::Resumed => {
                            if respect_surface_create {
                                if let Err(err) = native_user.window_created_ntfy(&mut window) {
                                    log::error!(target: "native", "{err}");
                                    window.destroy.set(true);
                                }
                                // TODO
                                let inner_size = window.borrow_window().inner_size().clamp(
                                    PhysicalSize {
                                        width: MIN_WINDOW_WIDTH,
                                        height: MIN_WINDOW_HEIGHT,
                                    },
                                    PhysicalSize {
                                        width: u32::MAX,
                                        height: u32::MAX,
                                    },
                                );
                                native_user.resized(
                                    &mut window,
                                    inner_size.width,
                                    inner_size.height,
                                );
                                native_user.window_options_changed(window.window_options());
                            }
                        }
                        Event::Suspended => {
                            if respect_surface_create {
                                if let Err(err) = native_user.window_destroyed_ntfy(&mut window) {
                                    log::error!(target: "native", "{err}");
                                    window.destroy.set(true);
                                }
                            }
                        }
                        Event::UserEvent(_) => {}
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
                                    window.mouse.try_set_mouse_grab(&window.window).is_err()
                                }
                            });
                        }
                    }
                }
                NativeUser::Wait => {
                    if matches!(event, Event::Resumed) {
                        match native_user_loading
                            .take()
                            .ok_or_else(|| anyhow!("user loading was already consumed"))
                            .and_then(|loading| F::new(loading, &mut window))
                        {
                            Ok(user) => {
                                native_user_opt = NativeUser::Some(user);
                            }
                            Err(err) => {
                                log::error!(target: "native", "{err}");
                                window.destroy.set(true);
                            }
                        }
                    }
                }
                _ => {
                    // ignore
                }
            }

            if window.destroy.get() {
                event_loop.exit();
                if let NativeUser::Some(native_user) = native_user_opt.take() {
                    native_user.destroy();
                }
            }
        })?)
    }
}
