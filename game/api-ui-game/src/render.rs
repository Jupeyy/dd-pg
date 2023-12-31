use api::{GRAPHICS, IO, RUNTIME_THREAD_POOL};
use api_ui::types::UIWinitWrapper;
use base_log::log::SystemLog;
use client_containers::skins::SkinContainer;
use client_render_base::render::tee::RenderTee;
use egui::FullOutput;
use graphics::graphics::Graphics;
use ui_base::{types::UINativePipe, ui::UI, ui_render::render_ui_2};

static mut SYS_LOG: once_cell::unsync::Lazy<SystemLog> =
    once_cell::unsync::Lazy::new(SystemLog::new);

static mut SKIN_CONTAINER: once_cell::unsync::Lazy<SkinContainer> =
    once_cell::unsync::Lazy::new(|| {
        let default_skin = SkinContainer::load(
            unsafe { &GRAPHICS }.get_graphics_mt(),
            "default",
            unsafe { &IO },
            unsafe { &RUNTIME_THREAD_POOL },
        );
        SkinContainer::new(
            unsafe { IO.clone() },
            unsafe { RUNTIME_THREAD_POOL.clone() },
            default_skin,
            unsafe { &SYS_LOG },
            "skin-container",
            unsafe { &GRAPHICS },
        )
    });

static mut TEE_RENDER: once_cell::unsync::Lazy<RenderTee> =
    once_cell::unsync::Lazy::new(|| RenderTee::new(unsafe { &mut GRAPHICS }));

#[no_mangle]
pub fn mod_render_ui(
    ui: &mut UI<UIWinitWrapper>,
    native_pipe: &mut UINativePipe<UIWinitWrapper>,
    full_output: FullOutput,
    screen_rect: &egui::Rect,
    zoom_level: f32,
    graphics: &mut Graphics,
    as_stencil: bool,
) {
    render_ui_2(
        ui,
        native_pipe,
        unsafe { &mut *SKIN_CONTAINER },
        unsafe { &mut *TEE_RENDER },
        full_output,
        screen_rect,
        zoom_level,
        graphics,
        as_stencil,
    )
}
