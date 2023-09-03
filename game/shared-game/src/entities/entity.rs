pub mod entity {
    use base_log::log::SystemLogGroup;
    use math::math::{round_to_int, vector::vec2};
    use pool::recycle::Recycle;

    use shared_base::{game_types::TGameElementID, reuseable::ReusableCore, types::GameTickType};

    use super::super::super::collision::collision::Collision;
    pub trait EntityInterface<
        C: Copy + Clone + bincode::Encode + bincode::Decode,
        R: ReusableCore + bincode::Encode + bincode::Decode,
        P,
    >
    {
        fn pre_tick(pipe: &mut P);
        fn tick(pipe: &mut P);
        fn tick_deferred(pipe: &mut P);

        /// split the entity to all main objects it contains of
        /// core (must be Copy'able)
        /// reusable core (must implement `ReusableCore`)
        fn split_mut(&mut self, index: usize) -> (&mut Entity, &mut C, &mut Recycle<R>);

        /// get a specific core (0: non-prediction, 1: prediction)
        fn get_core_at_index(&self, index: usize) -> &C;

        /// get a specific core (0: non-prediction, 1: prediction)
        fn get_core_at_index_mut(&mut self, index: usize) -> &mut C;

        /// get reusable cores
        fn get_reusable_cores_mut(&mut self) -> &mut [Recycle<R>];

        /// get a specific reusable core (0: non-prediction, 1: prediction)
        fn get_reusable_core_at_index(&self, index: usize) -> &Recycle<R>;

        /// get a specific reusable core (0: non-prediction, 1: prediction)
        fn get_reusable_core_at_index_mut(&mut self, index: usize) -> &mut Recycle<R>;

        /// get the non prediction core
        fn get_core(&self) -> &C {
            self.get_core_at_index(0)
        }

        /// get the non prediction core as mutable
        fn get_core_mut(&mut self) -> &mut C {
            self.get_core_at_index_mut(0)
        }

        /// get the prediction core
        fn get_prediction_core(&self) -> &C {
            self.get_core_at_index(1)
        }

        /// get the prediction core as mutable
        fn get_prediction_core_mut(&mut self) -> &mut C {
            self.get_core_at_index_mut(1)
        }

        /// get the non prediction reusable core
        fn get_reusable_core(&self) -> &Recycle<R> {
            self.get_reusable_core_at_index(0)
        }

        /// get the non prediction reusable core as mutable
        fn get_reusable_core_mut(&mut self) -> &mut Recycle<R> {
            self.get_reusable_core_at_index_mut(0)
        }

        /// get the prediction reusable core
        fn get_prediction_reusable_core(&self) -> &Recycle<R> {
            self.get_reusable_core_at_index(1)
        }

        /// get the prediction reusable core as mutable
        fn get_prediction_reusable_core_mut(&mut self) -> &mut Recycle<R> {
            self.get_reusable_core_at_index_mut(1)
        }

        /// copy the core
        fn copy_core(&mut self, dst_index: usize, src_index: usize) {
            *self.get_core_at_index_mut(dst_index) = *self.get_core_at_index(src_index);
        }

        /// copy the core
        fn copy_reusable_core(&mut self, dst_index: usize, src_index: usize) {
            let cores = self.get_reusable_cores_mut();
            let (cores_old, cores_new) = cores.split_at_mut(dst_index);
            if src_index >= dst_index {
                let (cores_old, cores_new) = cores_new.split_at_mut(src_index);
                cores_old[0].copy_clone_from(&cores_new[0]);
            } else {
                cores_new[0].copy_clone_from(&cores_old[src_index]);
            }
        }
    }

    #[derive(Debug)]
    pub struct Entity {
        pub game_element_id: TGameElementID,

        pub entity_events: Vec<EntitiyEvent>,

        pub(crate) _logger: SystemLogGroup,
    }

    impl Entity {
        pub fn new(game_el_id: &TGameElementID, logger: SystemLogGroup) -> Self {
            Self {
                game_element_id: game_el_id.clone(),

                entity_events: Default::default(),

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

    #[derive(Clone, Debug)]
    pub enum EntitiyEvent {
        Die {
            pos: vec2,
            respawns_at_tick: Option<GameTickType>,
        },
        Projectile {
            pos: vec2,
            dir: vec2,
        },
        Sound {
            // TODO:
        },
        Explosion {
            // TODO:
        },
    }

    pub fn calc_pos(pos: &mut vec2, vel: &vec2, curvature: f32, speed: f32, mut time: f32) {
        time *= speed;
        pos.x += vel.x * time;
        pos.y += vel.y * time + curvature / 10000.0 * (time * time);
    }
}
