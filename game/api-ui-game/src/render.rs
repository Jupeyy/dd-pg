use std::cell::RefCell;

use api::{GRAPHICS, IO, RUNTIME_THREAD_POOL, SOUND};
use base_log::log::SystemLog;
use client_containers_new::skins::{SkinContainer, SKIN_CONTAINER_PATH};
use client_render_base::render::tee::RenderTee;
use egui::FullOutput;
use graphics::graphics::graphics::Graphics;
use ui_base::{ui::UI, ui_render::render_ui_2};

static mut SYS_LOG: once_cell::unsync::Lazy<RefCell<SystemLog>> =
    once_cell::unsync::Lazy::new(|| RefCell::new(SystemLog::new()));

static mut SKIN_CONTAINER: once_cell::unsync::Lazy<SkinContainer> =
    once_cell::unsync::Lazy::new(|| {
        let default_skin =
            SkinContainer::load_default(unsafe { &IO.borrow() }, SKIN_CONTAINER_PATH.as_ref());
        let scene = unsafe { &SOUND.borrow() }.scene_handle.create();
        SkinContainer::new(
            unsafe { IO.borrow().clone() },
            RUNTIME_THREAD_POOL.clone(),
            default_skin,
            unsafe { &SYS_LOG.borrow() },
            None,
            None,
            "skin-container",
            unsafe { &GRAPHICS.borrow() },
            unsafe { &SOUND.borrow() },
            &scene,
            SKIN_CONTAINER_PATH.as_ref(),
        )
    });

static mut TEE_RENDER: once_cell::unsync::Lazy<RenderTee> =
    once_cell::unsync::Lazy::new(|| RenderTee::new(unsafe { &mut GRAPHICS.borrow_mut() }));

#[no_mangle]
pub fn mod_render_ui(
    ui: &mut UI,
    full_output: FullOutput,
    screen_rect: &egui::Rect,
    zoom_level: f32,
    graphics: &mut Graphics,
    as_stencil: bool,
) -> egui::PlatformOutput {
    render_ui_2(
        ui,
        unsafe { &mut *SKIN_CONTAINER },
        unsafe { &mut *TEE_RENDER },
        full_output,
        screen_rect,
        zoom_level,
        &graphics.backend_handle,
        &graphics.texture_handle,
        &graphics.stream_handle,
        as_stencil,
    )
}
