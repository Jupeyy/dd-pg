pub mod entity {
    use base_log::log::SystemLogGroup;
    use math::math::{round_to_int, vector::vec2};
    use pool::recycle::Recycle;

    use shared_base::{game_types::TGameElementID, reuseable::ReusableCore};

    use super::super::super::collision::collision::Collision;
    pub trait EntityInterface<
        C: Copy + Clone + bincode::Encode + bincode::Decode + 'static,
        R: ReusableCore + bincode::Encode + bincode::Decode + 'static,
        P,
    >
    {
        fn pre_tick(&mut self, pipe: &mut P);
        fn tick(&mut self, pipe: &mut P);
        fn tick_deferred(&mut self, pipe: &mut P);

        /// split the entity to all main objects it contains of
        /// the entity
        /// core (must be Copy'able)
        /// reusable core (must implement `ReusableCore`)
        fn split(&self) -> (&Entity, &C, &Recycle<R>);

        /// split the entity to all main objects it contains of
        /// the entity
        /// core (must be Copy'able)
        /// reusable core (must implement `ReusableCore`)
        fn split_mut(&mut self) -> (&mut Entity, &mut C, &mut Recycle<R>);

        /// get the non prediction core
        fn get_core(&self) -> &C {
            self.split().1
        }

        /// get the non prediction core as mutable
        fn get_core_mut(&mut self) -> &mut C {
            self.split_mut().1
        }

        /// get the non prediction reusable core
        fn get_reusable_core(&self) -> &Recycle<R> {
            self.split().2
        }

        /// get the non prediction reusable core as mutable
        fn get_reusable_core_mut(&mut self) -> &mut Recycle<R> {
            self.split_mut().2
        }

        /// copy the core
        fn copy_core(&mut self, other: &Self) {
            *self.split_mut().1 = *other.split().1;
        }

        /// copy the core
        fn copy_reusable_core(&mut self, other: &Self) {
            let (_, _, core) = self.split_mut();
            let (_, _, other_core) = other.split();
            core.copy_clone_from(other_core);
        }
    }

    #[derive(Debug)]
    pub struct Entity {
        pub game_element_id: TGameElementID,

        pub(crate) _logger: SystemLogGroup,
    }

    impl Entity {
        pub fn new(game_el_id: &TGameElementID, logger: SystemLogGroup) -> Self {
            Self {
                game_element_id: game_el_id.clone(),

                _logger: logger,
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
