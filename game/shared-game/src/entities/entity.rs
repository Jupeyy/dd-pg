pub mod entity {
    use base_log::log::SystemLogGroup;
    use game_interface::types::game::GameEntityId;
    use hiarc::Hiarc;
    use math::math::{round_to_int, vector::vec2};

    use serde::{de::DeserializeOwned, Serialize};
    use shared_base::reusable::ReusableCore;

    #[derive(Debug, PartialEq, Eq)]
    pub enum EntityTickResult {
        None,
        RemoveEntity,
    }

    use super::super::super::collision::collision::Collision;
    pub trait EntityInterface<
        C: Copy + Clone + Serialize + DeserializeOwned + 'static,
        R: ReusableCore + Serialize + DeserializeOwned + 'static,
        P,
    >
    {
        #[must_use]
        fn pre_tick(&mut self, pipe: &mut P) -> EntityTickResult;
        #[must_use]
        fn tick(&mut self, pipe: &mut P) -> EntityTickResult;
        #[must_use]
        fn tick_deferred(&mut self, pipe: &mut P) -> EntityTickResult;
        /// The entity dropped as a result of a logic side effect (e.g. snapshots),
        /// and not because of game logic.
        fn drop_silent(&mut self);
    }

    #[derive(Debug, Hiarc)]
    pub struct Entity {
        pub game_element_id: GameEntityId,

        pub(crate) _logger: SystemLogGroup,

        pub drop_silent: bool,
    }

    impl Entity {
        pub fn new(game_el_id: &GameEntityId, logger: SystemLogGroup) -> Self {
            Self {
                game_element_id: game_el_id.clone(),

                _logger: logger,

                drop_silent: false,
            }
        }

        pub fn outside_of_playfield(check_pos: &vec2, collision: &Collision) -> bool {
            let rx = round_to_int(check_pos.x) / 32;
            let ry = round_to_int(check_pos.y) / 32;
            (rx < -200 || rx >= collision.get_playfield_width() as i32 + 200)
                || (ry < -200 || ry >= collision.get_playfield_height() as i32 + 200)
        }
    }

    pub fn calc_pos_and_vel(
        pos: &mut vec2,
        vel: &mut vec2,
        curvature: f32,
        speed: f32,
        mut time: f32,
    ) {
        time *= speed;
        pos.x += vel.x * time;

        let curvature = curvature / 10000.0;
        pos.y += vel.y * time + curvature * (time * time);
        vel.y += curvature * 2.0 * time; // derivation of time to above
    }
}
