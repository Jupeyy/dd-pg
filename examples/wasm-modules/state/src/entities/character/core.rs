use api_macros::character_core_mod;

#[character_core_mod("../../../")]
pub mod character_core {

    impl Core {
        // half gravity mod
        fn get_gravity(collision: &Collision, pos: &vec2) -> f32 {
            let tuning = collision.get_tune_at(pos);
            tuning.gravity * 0.5
        }
    }
}
