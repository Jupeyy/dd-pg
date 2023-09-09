use api_macros::character_core_mod;

#[character_core_mod("../../../")]
pub mod character_core {
    pub trait CorePhysics {
        // half gravity mod
        fn get_gravity(core: &Core) -> f32 {
            core.tuning.gravity * 0.5
        }
    }
}
