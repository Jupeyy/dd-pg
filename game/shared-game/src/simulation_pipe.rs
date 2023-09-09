pub mod simulation_pipe {
    use shared_base::{
        game_types::TGameElementID, id_gen::IDGenerator, network::messages::MsgObjPlayerInput,
        types::GameTickType,
    };

    use crate::{
        entities::{
            character::character::Characters,
            entity::entity::{Entity, EntityInterface},
            flag::flag::Flag,
            laser::laser::Laser,
            pickup::pickup::Pickup,
            projectile::projectile::{PoolProjectileReusableCore, Projectile, ProjectileCore},
        },
        world::world::WorldPool,
    };

    use super::super::{
        collision::collision::Collision,
        entities::{
            character::character::{
                Character, CharacterCore, CharacterReusableCore, PoolCharacterReusableCore,
            },
            character_core::character_core::{Core, CoreReusable},
        },
        player::player::{Player, PlayerRemoveInfo, Players},
    };

    /// simulation events are events
    /// that should be handled by a upper component
    /// it's also useful to cleanly split prediction code
    /// from actual ticks.. prediction code can simply ignore
    /// these events
    #[derive(Clone)]
    pub enum SimulationEvents {
        PlayerCharacterRemoved {
            id: TGameElementID,
            remove_info: PlayerRemoveInfo,
        },
    }

    pub trait SimulationPlayerInput {
        fn get_input(&self, player_id: &TGameElementID) -> Option<&MsgObjPlayerInput>;
    }

    pub struct SimulationPipe<'a> {
        pub collision: &'a Collision,
    }

    impl<'a> SimulationPipe<'a> {
        pub fn new(collision: &'a Collision) -> Self {
            Self {
                collision: collision,
            }
        }
    }

    pub struct SimulationPipeStage<'a> {
        pub prev_core_index: usize,
        pub next_core_index: usize,

        pub players: &'a Players,

        // should only be true inside a client's simulation pipe
        pub is_prediction: bool,

        pub collision: &'a Collision,

        pub stage_id: &'a TGameElementID,

        pub cur_tick: GameTickType,

        pub simulation_events: &'a mut Vec<SimulationEvents>,

        pub id_generator: &'a mut IDGenerator,

        pub world_pool: &'a mut WorldPool,
    }

    impl<'a> SimulationPipeStage<'a> {
        pub fn new(
            prev_core_index: usize,
            next_core_index: usize,
            players: &'a Players,
            is_prediction: bool,
            collision: &'a Collision,
            stage_id: &'a TGameElementID,
            cur_tick: GameTickType,
            simulation_events: &'a mut Vec<SimulationEvents>,
            id_generator: &'a mut IDGenerator,
            world_pool: &'a mut WorldPool,
        ) -> Self {
            Self {
                next_core_index,
                prev_core_index,
                players,
                is_prediction,
                collision,
                stage_id,
                cur_tick,
                simulation_events,
                id_generator,
                world_pool,
            }
        }
    }

    pub trait SimulationPipeCharactersGetter {
        fn get_character(&mut self) -> &mut Character;
        fn get_character_id(&self) -> &TGameElementID;

        fn get_other_character_id_and_cores_iter(
            &self,
            for_each_func: &mut dyn FnMut(&TGameElementID, &Core),
        );
        fn get_other_character_id_and_cores_iter_mut(
            &mut self,
            for_each_func: &mut dyn FnMut(&TGameElementID, &mut Core, &mut CoreReusable),
        );
        fn get_other_character_core_by_id(&self, other_char_id: &TGameElementID) -> &Core;
        fn get_other_character_by_id_mut(
            &mut self,
            other_char_id: &TGameElementID,
        ) -> &mut Character;
    }

    pub struct SimulationPipeCharacter<'a> {
        pub cur_core_index: usize,

        pub character_player: &'a Player,

        pub player_inputs: &'a dyn SimulationPlayerInput,
        pub characters: &'a mut dyn SimulationPipeCharactersGetter,

        pub collision: &'a Collision,

        pub cur_tick: GameTickType,
    }

    impl<'a> SimulationPipeCharacter<'a> {
        pub fn new(
            cur_core_index: usize,
            character_player: &'a Player,
            player_inputs: &'a dyn SimulationPlayerInput,
            characters: &'a mut dyn SimulationPipeCharactersGetter,
            collision: &'a Collision,
            cur_tick: GameTickType,
        ) -> Self {
            Self {
                cur_core_index,
                character_player,
                player_inputs,
                characters,
                collision,
                cur_tick,
            }
        }

        pub fn get_ent_and_core_mut(
            &mut self,
        ) -> (
            &mut Entity,
            &mut CharacterCore,
            &mut PoolCharacterReusableCore,
        ) {
            self.characters
                .get_character()
                .split_mut(self.cur_core_index)
        }

        pub fn get_split_mut(
            &mut self,
        ) -> (
            &mut Entity,
            &mut CharacterCore,
            &mut CharacterReusableCore,
            GameTickType,
            &Collision,
            &Player,
        ) {
            let (ent, core, reusable_core) = self
                .characters
                .get_character()
                .split_mut(self.cur_core_index);
            (
                ent,
                core,
                reusable_core,
                self.cur_tick,
                self.collision,
                self.character_player,
            )
        }
    }

    pub struct SimulationPipeProjectileCharacters<'a> {
        pub characters: &'a mut Characters,
        pub owner_character: TGameElementID,
    }

    impl<'a> SimulationPipeProjectileCharacters<'a> {
        pub fn get_characters_except_owner(&mut self) -> impl Iterator<Item = &mut Character> {
            self.characters
                .values_mut()
                .filter(|char| char.base.game_element_id != self.owner_character)
        }
    }

    pub struct SimulationPipeProjectileEntity<'a> {
        pub cur_core_index: usize,
        pub projectile: &'a mut Projectile,
    }

    impl<'a> SimulationPipeProjectileEntity<'a> {
        pub fn get_ent_and_core_mut(
            &mut self,
        ) -> (
            &mut Entity,
            &mut ProjectileCore,
            &mut PoolProjectileReusableCore,
        ) {
            self.projectile.split_mut(self.cur_core_index)
        }
    }

    pub struct SimulationPipeProjectile<'a> {
        pub cur_core_index: usize,
        pub collision: &'a Collision,

        pub cur_tick: GameTickType,

        pub projectile: SimulationPipeProjectileEntity<'a>,
        pub characters_helper: SimulationPipeProjectileCharacters<'a>,
    }

    impl<'a> SimulationPipeProjectile<'a> {
        pub fn new(
            cur_core_index: usize,
            collision: &'a Collision,
            projectile: &'a mut Projectile,
            characters: &'a mut Characters,
            cur_tick: GameTickType,
            owner_character: TGameElementID,
        ) -> Self {
            Self {
                cur_core_index,
                collision,
                projectile: SimulationPipeProjectileEntity {
                    cur_core_index: cur_core_index,
                    projectile,
                },
                cur_tick,
                characters_helper: SimulationPipeProjectileCharacters {
                    characters,
                    owner_character,
                },
            }
        }
    }

    pub struct SimulationPipePickup<'a> {
        pub cur_core_index: usize,
        pub pickup: &'a mut Pickup,
    }

    impl<'a> SimulationPipePickup<'a> {
        pub fn new(cur_core_index: usize, pickup: &'a mut Pickup) -> Self {
            Self {
                cur_core_index,
                pickup,
            }
        }
    }

    pub struct SimulationPipeFlag<'a> {
        pub cur_core_index: usize,
        pub flag: &'a mut Flag,
    }

    impl<'a> SimulationPipeFlag<'a> {
        pub fn new(cur_core_index: usize, flag: &'a mut Flag) -> Self {
            Self {
                cur_core_index,
                flag,
            }
        }
    }

    pub struct SimulationPipeLaser<'a> {
        pub cur_core_index: usize,
        pub laser: &'a mut Laser,
    }

    impl<'a> SimulationPipeLaser<'a> {
        pub fn new(cur_core_index: usize, laser: &'a mut Laser) -> Self {
            Self {
                cur_core_index,
                laser,
            }
        }
    }
}
