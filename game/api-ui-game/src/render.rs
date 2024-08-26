use api::{GRAPHICS, IO, RUNTIME_THREAD_POOL, SOUND};
use client_containers::{
    emoticons::{EmoticonsContainer, EMOTICONS_CONTAINER_PATH},
    skins::{SkinContainer, SKIN_CONTAINER_PATH},
    weapons::{WeaponContainer, WEAPON_CONTAINER_PATH},
};

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

/// made to be easy to use for API stuff
pub fn create_emoticons_container() -> EmoticonsContainer {
    let default_emoticons = EmoticonsContainer::load_default(
        unsafe { &IO.borrow() },
        EMOTICONS_CONTAINER_PATH.as_ref(),
    );
    let scene = unsafe { &SOUND.borrow() }.scene_handle.create();
    EmoticonsContainer::new(
        unsafe { IO.borrow().clone() },
        RUNTIME_THREAD_POOL.clone(),
        default_emoticons,
        None,
        None,
        "emoticons-container",
        unsafe { &GRAPHICS.borrow() },
        unsafe { &SOUND.borrow() },
        &scene,
        EMOTICONS_CONTAINER_PATH.as_ref(),
    )
}

/// made to be easy to use for API stuff
pub fn create_weapon_container() -> WeaponContainer {
    let default_weapon =
        WeaponContainer::load_default(unsafe { &IO.borrow() }, WEAPON_CONTAINER_PATH.as_ref());
    let scene = unsafe { &SOUND.borrow() }.scene_handle.create();
    WeaponContainer::new(
        unsafe { IO.borrow().clone() },
        RUNTIME_THREAD_POOL.clone(),
        default_weapon,
        None,
        None,
        "weapon-container",
        unsafe { &GRAPHICS.borrow() },
        unsafe { &SOUND.borrow() },
        &scene,
        WEAPON_CONTAINER_PATH.as_ref(),
    )
}
