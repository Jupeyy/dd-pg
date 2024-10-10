use api::{GRAPHICS, IO, RUNTIME_THREAD_POOL, SOUND};
use client_ui::main_menu::theme_container::{ThemeContainer, THEME_CONTAINER_PATH};

pub mod page;
pub mod profiles;

/// made to be easy to use for API stuff
pub fn create_theme_container() -> ThemeContainer {
    let default_item =
        ThemeContainer::load_default(unsafe { &IO.borrow() }, THEME_CONTAINER_PATH.as_ref());
    let scene = unsafe { &SOUND.borrow() }
        .scene_handle
        .create(Default::default());
    ThemeContainer::new(
        unsafe { IO.borrow().clone() },
        RUNTIME_THREAD_POOL.clone(),
        default_item,
        None,
        None,
        "theme-container",
        unsafe { &GRAPHICS.borrow() },
        unsafe { &SOUND.borrow() },
        &scene,
        THEME_CONTAINER_PATH.as_ref(),
    )
}
