pub mod stage {
    use std::{
        ops::{Deref, DerefMut},
        sync::Arc,
    };

    use base_log::log::SystemLog;
    use hashlink::LinkedHashMap;
    use math::math::vector::vec2;
    use shared_base::{
        game_types::TGameElementID,
        network::messages::{MsgObjPlayerInfo, WeaponType},
        types::GameTickType,
    };

    use crate::{
        match_manager::match_manager::{MatchManager, MatchState},
        types::types::GameOptions,
    };

    use super::super::{
        simulation_pipe::simulation_pipe::SimulationPipeStage,
        world::world::{GameWorld, WorldPool},
    };

    /**
     * The game stage represents a well split state of a complete world, which is useful to
     * have multiple people being able to play on the same server without touching each other
     * It's there to implement ddrace teams.
     */
    pub struct GameStage {
        pub world: GameWorld,
        pub match_manager: MatchManager,
        pub stage_index: usize,

        pub game_element_id: TGameElementID,
    }

    impl GameStage {
        pub fn new(
            stage_index: usize,
            game_element_id: TGameElementID,
            world_pool: &WorldPool,
            game_options: GameOptions,
            log: &Arc<SystemLog>,
        ) -> Self {
            Self {
                world: GameWorld::new(world_pool, log),
                match_manager: MatchManager::new(game_element_id, game_options),
                stage_index: stage_index,

                game_element_id,
            }
        }

        pub fn tick(&mut self, pipe: &mut SimulationPipeStage) {
            if let MatchState::Running = self.match_manager.state {
                self.world.tick(pipe);
            }
            if self.match_manager.handle_events(
                pipe.cur_tick,
                &mut self.world,
                pipe.simulation_events,
            ) {
                self.world = GameWorld::new(&self.world.world_pool, &self.world.log)
            }
        }
    }

    #[derive(Default)]
    pub struct Stages(LinkedHashMap<TGameElementID, GameStage>);

    impl Deref for Stages {
        type Target = LinkedHashMap<TGameElementID, GameStage>;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl DerefMut for Stages {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    impl Stages {
        pub fn contains_character(
            &mut self,
            stage_id: &TGameElementID,
            character_id: &TGameElementID,
        ) -> bool {
            self.get(stage_id)
                .unwrap()
                .world
                .characters
                .contains_key(character_id)
        }

        pub(crate) fn insert_new_character_to_stage(
            &mut self,
            stage_id: &TGameElementID,
            character_id: TGameElementID,
            player_info: MsgObjPlayerInfo,
        ) {
            let stage = self.get_mut(stage_id).unwrap();
            stage
                .world
                .add_character(character_id, player_info, &stage.match_manager.game_options);
        }

        pub fn get_stage_by_id_mut(&mut self, id: &TGameElementID) -> &mut GameStage {
            self.get_mut(id).unwrap()
        }

        pub fn contains_projectile(
            &mut self,
            stage_id: &TGameElementID,
            projectile_id: &TGameElementID,
        ) -> bool {
            self.get(stage_id)
                .unwrap()
                .world
                .get_projectiles()
                .contains_key(projectile_id)
        }

        pub(crate) fn insert_new_projectile_to_stage(
            &mut self,
            stage_id: &TGameElementID,
            projectile_id: TGameElementID,
            owner_character_id: TGameElementID,
            pos: &vec2,
            direction: &vec2,
            life_span: i32,
            damage: u32,
            force: f32,
            start_tick: GameTickType,
            explosive: bool,
            ty: WeaponType,
        ) {
            self.get_mut(stage_id).unwrap().world.insert_new_projectile(
                projectile_id,
                owner_character_id,
                pos,
                direction,
                life_span,
                damage,
                force,
                start_tick,
                explosive,
                ty,
            );
        }

        pub fn contains_laser(
            &mut self,
            stage_id: &TGameElementID,
            laser_id: &TGameElementID,
        ) -> bool {
            self.get(stage_id)
                .unwrap()
                .world
                .get_lasers()
                .contains_key(laser_id)
        }

        pub(crate) fn insert_new_laser_to_stage(
            &mut self,
            stage_id: &TGameElementID,
            laser_id: TGameElementID,
            owner_character_id: TGameElementID,

            pos: &vec2,
            dir: &vec2,
            start_tick: GameTickType,
            start_energy: f32,
            can_hit_others: bool,
            can_hit_own: bool,
        ) {
            self.get_mut(stage_id).unwrap().world.insert_new_laser(
                laser_id,
                owner_character_id,
                pos,
                dir,
                start_tick,
                start_energy,
                can_hit_others,
                can_hit_own,
            );
        }

        pub fn contains_pickup(
            &mut self,
            stage_id: &TGameElementID,
            pickup_id: &TGameElementID,
        ) -> bool {
            self.get(stage_id)
                .unwrap()
                .world
                .get_pickups()
                .contains_key(pickup_id)
        }

        pub(crate) fn insert_new_pickup_to_stage(
            &mut self,
            stage_id: &TGameElementID,
            pickup_id: TGameElementID,
            pos: &vec2,
        ) {
            self.get_mut(stage_id)
                .unwrap()
                .world
                .insert_new_pickup(pickup_id, pos);
        }

        pub fn contains_flag(
            &mut self,
            stage_id: &TGameElementID,
            flag_id: &TGameElementID,
        ) -> bool {
            self.get(stage_id)
                .unwrap()
                .world
                .get_flags()
                .contains_key(flag_id)
        }

        pub(crate) fn insert_new_flag_to_stage(
            &mut self,
            stage_id: &TGameElementID,
            flag_id: TGameElementID,
            pos: &vec2,
        ) {
            self.get_mut(stage_id)
                .unwrap()
                .world
                .insert_new_flag(flag_id, pos);
        }
    }
}
