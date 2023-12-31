pub mod simulation_pipe {
    use bincode::{Decode, Encode};
    use hiarc_macro::{hiarc_safer_rc_refcell, Hiarc};
    use math::math::vector::vec2;
    use shared_base::{game_types::TGameElementID, id_gen::IDGenerator, types::GameTickType};

    use crate::{
        entities::character::character::Characters, events::events::CharacterEvent,
        world::world::WorldPool,
    };

    use super::super::{
        collision::collision::Collision,
        entities::{
            character::character::Character,
            character_core::character_core::{Core, CoreReusable},
        },
    };

    #[derive(Debug, Clone, Encode, Decode)]
    pub enum SimulationEventsWorld {
        Character {
            player_id: TGameElementID,
            ev: CharacterEvent,
        },
    }

    /// simulation events are events that should be
    /// handled by a upper component
    /// it's also useful to cleanly split prediction code
    /// from actual ticks.. prediction code can simply ignore
    /// these events
    #[derive(Debug, Clone, Encode, Decode)]
    pub enum SimulationEvent {
        World {
            stage_id: TGameElementID,
            ev: SimulationEventsWorld,
        },
    }

    #[hiarc_safer_rc_refcell]
    #[derive(Debug, Default, Hiarc)]
    pub struct SimulationEvents {
        events: Vec<SimulationEvent>,
    }

    #[hiarc_safer_rc_refcell]
    impl SimulationEvents {
        pub fn push(&mut self, ev: SimulationEvent) {
            self.events.push(ev);
        }
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
        // should only be true inside a client's simulation pipe
        pub is_prediction: bool,

        pub collision: &'a Collision,

        pub stage_id: &'a TGameElementID,

        pub cur_tick: GameTickType,

        pub simulation_events: &'a mut Vec<SimulationEvent>,

        pub id_generator: &'a mut IDGenerator,

        pub world_pool: &'a mut WorldPool,
    }

    impl<'a> SimulationPipeStage<'a> {
        pub fn new(
            is_prediction: bool,
            collision: &'a Collision,
            stage_id: &'a TGameElementID,
            cur_tick: GameTickType,
            simulation_events: &'a mut Vec<SimulationEvent>,
            id_generator: &'a mut IDGenerator,
            world_pool: &'a mut WorldPool,
        ) -> Self {
            Self {
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
        fn for_other_characters_in_range(
            &mut self,
            char_pos: &vec2,
            radius: f32,
            for_each_func: &mut dyn FnMut(&mut Character),
        );
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
        pub characters: &'a mut dyn SimulationPipeCharactersGetter,

        pub collision: &'a Collision,

        pub cur_tick: GameTickType,
    }

    impl<'a> SimulationPipeCharacter<'a> {
        pub fn new(
            characters: &'a mut dyn SimulationPipeCharactersGetter,
            collision: &'a Collision,
            cur_tick: GameTickType,
        ) -> Self {
            Self {
                characters,
                collision,
                cur_tick,
            }
        }
    }

    pub struct SimulationPipeCharacters<'a> {
        pub characters: &'a mut Characters,
        pub owner_character: TGameElementID,
    }

    impl<'a> SimulationPipeCharacters<'a> {
        pub fn get_characters_except_owner(&mut self) -> impl Iterator<Item = &mut Character> {
            self.characters
                .values_mut()
                .filter(|char| char.base.game_element_id != self.owner_character)
        }
        pub fn get_characters(&mut self) -> impl Iterator<Item = &mut Character> {
            self.characters.values_mut()
        }
        pub fn get_owner_character_it(&mut self) -> impl Iterator<Item = &mut Character> {
            [self.characters.get_mut(&self.owner_character).unwrap()].into_iter()
        }
        pub fn get_owner_character(&mut self) -> &mut Character {
            self.characters.get_mut(&self.owner_character).unwrap()
        }
    }

    pub struct SimulationPipeProjectile<'a> {
        pub collision: &'a Collision,

        pub cur_tick: GameTickType,

        pub characters_helper: SimulationPipeCharacters<'a>,
    }

    impl<'a> SimulationPipeProjectile<'a> {
        pub fn new(
            collision: &'a Collision,
            characters: &'a mut Characters,
            cur_tick: GameTickType,
            owner_character: TGameElementID,
        ) -> Self {
            Self {
                collision,
                cur_tick,
                characters_helper: SimulationPipeCharacters {
                    characters,
                    owner_character,
                },
            }
        }
    }

    pub struct SimulationPipePickup {}

    impl SimulationPipePickup {
        pub fn new() -> Self {
            Self {}
        }
    }

    pub struct SimulationPipeFlag {}

    impl SimulationPipeFlag {
        pub fn new() -> Self {
            Self {}
        }
    }

    pub struct SimulationPipeLaser<'a> {
        pub cur_tick: GameTickType,

        pub collision: &'a Collision,

        pub characters_helper: SimulationPipeCharacters<'a>,
    }

    impl<'a> SimulationPipeLaser<'a> {
        pub fn new(
            cur_tick: GameTickType,
            collision: &'a Collision,
            characters: &'a mut Characters,
            owner_character: TGameElementID,
        ) -> Self {
            Self {
                cur_tick,

                collision,
                characters_helper: SimulationPipeCharacters {
                    characters,
                    owner_character,
                },
            }
        }
    }
}
