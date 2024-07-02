use api::{GRAPHICS, IO, RUNTIME_THREAD_POOL, SOUND};
use client_containers_new::skins::{SkinContainer, SKIN_CONTAINER_PATH};

/// made to be easy to use for API stuff
pub fn create_skin_container() -> SkinContainer {
    let default_skin =
        SkinContainer::load_default(unsafe { &IO.borrow() }, SKIN_CONTAINER_PATH.as_ref());
    let scene = unsafe { &SOUND.borrow() }.scene_handle.create();
    SkinContainer::new(
        unsafe { IO.borrow().clone() },
        RUNTIME_THREAD_POOL.clone(),
        default_skin,
        None,
        None,
        "skin-container",
        unsafe { &GRAPHICS.borrow() },
        unsafe { &SOUND.borrow() },
        &scene,
        SKIN_CONTAINER_PATH.as_ref(),
    )
}
