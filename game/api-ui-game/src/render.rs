use api::{GRAPHICS, IO, RUNTIME_THREAD_POOL, SOUND};
use client_containers::{
    ctf::{CtfContainer, CTF_CONTAINER_PATH},
    emoticons::{EmoticonsContainer, EMOTICONS_CONTAINER_PATH},
    entities::{EntitiesContainer, ENTITIES_CONTAINER_PATH},
    flags::{FlagsContainer, FLAGS_CONTAINER_PATH},
    freezes::{FreezeContainer, FREEZE_CONTAINER_PATH},
    game::{GameContainer, GAME_CONTAINER_PATH},
    hooks::{HookContainer, HOOK_CONTAINER_PATH},
    hud::{HudContainer, HUD_CONTAINER_PATH},
    ninja::{NinjaContainer, NINJA_CONTAINER_PATH},
    particles::{ParticlesContainer, PARTICLES_CONTAINER_PATH},
    skins::{SkinContainer, SKIN_CONTAINER_PATH},
    weapons::{WeaponContainer, WEAPON_CONTAINER_PATH},
};

/// made to be easy to use for API stuff
pub fn create_skin_container() -> SkinContainer {
    let default_skin =
        SkinContainer::load_default(unsafe { &IO.borrow() }, SKIN_CONTAINER_PATH.as_ref());
    let scene = unsafe { &SOUND.borrow() }
        .scene_handle
        .create(Default::default());
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
    let scene = unsafe { &SOUND.borrow() }
        .scene_handle
        .create(Default::default());
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
    let scene = unsafe { &SOUND.borrow() }
        .scene_handle
        .create(Default::default());
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

/// made to be easy to use for API stuff
pub fn create_flags_container() -> FlagsContainer {
    let default_flags =
        FlagsContainer::load_default(unsafe { &IO.borrow() }, FLAGS_CONTAINER_PATH.as_ref());
    let scene = unsafe { &SOUND.borrow() }
        .scene_handle
        .create(Default::default());
    FlagsContainer::new(
        unsafe { IO.borrow().clone() },
        RUNTIME_THREAD_POOL.clone(),
        default_flags,
        None,
        None,
        "flags-container",
        unsafe { &GRAPHICS.borrow() },
        unsafe { &SOUND.borrow() },
        &scene,
        FLAGS_CONTAINER_PATH.as_ref(),
    )
}

/// made to be easy to use for API stuff
pub fn create_hook_container() -> HookContainer {
    let default_hooks =
        HookContainer::load_default(unsafe { &IO.borrow() }, HOOK_CONTAINER_PATH.as_ref());
    let scene = unsafe { &SOUND.borrow() }
        .scene_handle
        .create(Default::default());
    HookContainer::new(
        unsafe { IO.borrow().clone() },
        RUNTIME_THREAD_POOL.clone(),
        default_hooks,
        None,
        None,
        "hooks-container",
        unsafe { &GRAPHICS.borrow() },
        unsafe { &SOUND.borrow() },
        &scene,
        HOOK_CONTAINER_PATH.as_ref(),
    )
}

/// made to be easy to use for API stuff
pub fn create_entities_container() -> EntitiesContainer {
    let default_item =
        EntitiesContainer::load_default(unsafe { &IO.borrow() }, ENTITIES_CONTAINER_PATH.as_ref());
    let scene = unsafe { &SOUND.borrow() }
        .scene_handle
        .create(Default::default());
    EntitiesContainer::new(
        unsafe { IO.borrow().clone() },
        RUNTIME_THREAD_POOL.clone(),
        default_item,
        None,
        None,
        "entities-container",
        unsafe { &GRAPHICS.borrow() },
        unsafe { &SOUND.borrow() },
        &scene,
        HOOK_CONTAINER_PATH.as_ref(),
    )
}

/// made to be easy to use for API stuff
pub fn create_freeze_container() -> FreezeContainer {
    let default_item =
        FreezeContainer::load_default(unsafe { &IO.borrow() }, FREEZE_CONTAINER_PATH.as_ref());
    let scene = unsafe { &SOUND.borrow() }
        .scene_handle
        .create(Default::default());
    FreezeContainer::new(
        unsafe { IO.borrow().clone() },
        RUNTIME_THREAD_POOL.clone(),
        default_item,
        None,
        None,
        "freeze-container",
        unsafe { &GRAPHICS.borrow() },
        unsafe { &SOUND.borrow() },
        &scene,
        HOOK_CONTAINER_PATH.as_ref(),
    )
}

/// made to be easy to use for API stuff
pub fn create_particles_container() -> ParticlesContainer {
    let default_item = ParticlesContainer::load_default(
        unsafe { &IO.borrow() },
        PARTICLES_CONTAINER_PATH.as_ref(),
    );
    let scene = unsafe { &SOUND.borrow() }
        .scene_handle
        .create(Default::default());
    ParticlesContainer::new(
        unsafe { IO.borrow().clone() },
        RUNTIME_THREAD_POOL.clone(),
        default_item,
        None,
        None,
        "particles-container",
        unsafe { &GRAPHICS.borrow() },
        unsafe { &SOUND.borrow() },
        &scene,
        HOOK_CONTAINER_PATH.as_ref(),
    )
}

/// made to be easy to use for API stuff
pub fn create_ninja_container() -> NinjaContainer {
    let default_item =
        NinjaContainer::load_default(unsafe { &IO.borrow() }, NINJA_CONTAINER_PATH.as_ref());
    let scene = unsafe { &SOUND.borrow() }
        .scene_handle
        .create(Default::default());
    NinjaContainer::new(
        unsafe { IO.borrow().clone() },
        RUNTIME_THREAD_POOL.clone(),
        default_item,
        None,
        None,
        "ninja-container",
        unsafe { &GRAPHICS.borrow() },
        unsafe { &SOUND.borrow() },
        &scene,
        HOOK_CONTAINER_PATH.as_ref(),
    )
}

/// made to be easy to use for API stuff
pub fn create_game_container() -> GameContainer {
    let default_item =
        GameContainer::load_default(unsafe { &IO.borrow() }, GAME_CONTAINER_PATH.as_ref());
    let scene = unsafe { &SOUND.borrow() }
        .scene_handle
        .create(Default::default());
    GameContainer::new(
        unsafe { IO.borrow().clone() },
        RUNTIME_THREAD_POOL.clone(),
        default_item,
        None,
        None,
        "game-container",
        unsafe { &GRAPHICS.borrow() },
        unsafe { &SOUND.borrow() },
        &scene,
        HOOK_CONTAINER_PATH.as_ref(),
    )
}

/// made to be easy to use for API stuff
pub fn create_hud_container() -> HudContainer {
    let default_item =
        HudContainer::load_default(unsafe { &IO.borrow() }, HUD_CONTAINER_PATH.as_ref());
    let scene = unsafe { &SOUND.borrow() }
        .scene_handle
        .create(Default::default());
    HudContainer::new(
        unsafe { IO.borrow().clone() },
        RUNTIME_THREAD_POOL.clone(),
        default_item,
        None,
        None,
        "hud-container",
        unsafe { &GRAPHICS.borrow() },
        unsafe { &SOUND.borrow() },
        &scene,
        HOOK_CONTAINER_PATH.as_ref(),
    )
}

/// made to be easy to use for API stuff
pub fn create_ctf_container() -> CtfContainer {
    let default_item =
        CtfContainer::load_default(unsafe { &IO.borrow() }, CTF_CONTAINER_PATH.as_ref());
    let scene = unsafe { &SOUND.borrow() }
        .scene_handle
        .create(Default::default());
    CtfContainer::new(
        unsafe { IO.borrow().clone() },
        RUNTIME_THREAD_POOL.clone(),
        default_item,
        None,
        None,
        "ctf-container",
        unsafe { &GRAPHICS.borrow() },
        unsafe { &SOUND.borrow() },
        &scene,
        HOOK_CONTAINER_PATH.as_ref(),
    )
}
