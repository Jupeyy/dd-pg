pub mod simulation_pipe {
    use std::marker::PhantomData;
    use std::ops::ControlFlow;

    use game_interface::events::GameWorldGlobalEvent;
    use game_interface::types::game::GameEntityId;
    use hashlink::{LinkedHashMap, LinkedHashSet};
    use hiarc::HiFnMut;
    use hiarc::{hiarc_safer_rc_refcell, Hiarc};
    use math::math::vector::vec2;
    use pool::{
        datatypes::{PoolLinkedHashMap, PoolVec},
        pool::Pool,
    };
    use serde::{Deserialize, Serialize};

    use crate::entities::character::character::CharactersView;
    use crate::entities::character::core::character_core::{Core, CoreReusable};
    use crate::entities::character::pos::character_pos::{
        CharacterPos, CharacterPositionPlayfield,
    };
    use crate::events::events::{FlagEvent, LaserEvent, PickupEvent, ProjectileEvent};
    use crate::{
        entities::character::character::Characters,
        events::events::CharacterEvent,
        world::world::{GameWorld, WorldPool},
    };

    use super::super::{
        collision::collision::Collision, entities::character::character::Character,
    };

    #[derive(Debug, Hiarc, Serialize, Deserialize)]
    pub enum SimulationEventWorldEntityType {
        Character {
            ev: CharacterEvent,
        },
        Projectile {
            id: GameEntityId,
            ev: ProjectileEvent,
        },
        Pickup {
            id: GameEntityId,
            ev: PickupEvent,
        },
        Flag {
            id: GameEntityId,
            ev: FlagEvent,
        },
        Laser {
            id: GameEntityId,
            ev: LaserEvent,
        },
    }

    #[derive(Debug, Hiarc, Serialize, Deserialize)]
    pub struct SimulationEventWorldEntity {
        pub owner_id: Option<GameEntityId>,
        pub ev: SimulationEventWorldEntityType,
    }

    #[hiarc_safer_rc_refcell]
    #[derive(Debug, Hiarc)]
    pub struct SimulationEntityEvents {
        events: PoolVec<SimulationEventWorldEntity>,
        pool: Pool<Vec<SimulationEventWorldEntity>>,
    }

    #[hiarc_safer_rc_refcell]
    impl Default for SimulationEntityEvents {
        fn default() -> Self {
            Self::new()
        }
    }

    #[hiarc_safer_rc_refcell]
    impl SimulationEntityEvents {
        pub fn new() -> Self {
            let pool = Pool::with_capacity(2);
            Self {
                events: pool.new(),
                pool,
            }
        }

        pub fn push(&mut self, owner_id: Option<GameEntityId>, ev: SimulationEventWorldEntityType) {
            self.events
                .push(SimulationEventWorldEntity { ev, owner_id });
        }

        pub fn take(&mut self) -> PoolVec<SimulationEventWorldEntity> {
            let events = self.pool.new();
            std::mem::replace(&mut self.events, events)
        }
    }

    #[derive(Debug, Hiarc, Serialize, Deserialize)]
    pub enum SimulationWorldEvent {
        Entity(SimulationEventWorldEntity),
        Global(GameWorldGlobalEvent),
    }

    pub type SimulationWorldEvents = PoolVec<SimulationWorldEvent>;

    #[hiarc_safer_rc_refcell]
    #[derive(Debug, Hiarc)]
    pub struct SimulationStageEvents {
        events: SimulationWorldEvents,
        pool: Pool<Vec<SimulationWorldEvent>>,

        // match manager should have higher hierarchy than world
        _py: PhantomData<GameWorld>,
    }

    #[hiarc_safer_rc_refcell]
    impl Default for SimulationStageEvents {
        fn default() -> Self {
            Self::new()
        }
    }

    #[hiarc_safer_rc_refcell]
    impl SimulationStageEvents {
        pub fn new() -> Self {
            let pool = Pool::with_capacity(2);
            Self {
                events: pool.new(),
                pool,

                _py: Default::default(),
            }
        }

        pub fn push(&mut self, ev: SimulationWorldEvent) {
            self.events.push(ev);
        }

        pub fn push_entity_evs(&mut self, mut evs: PoolVec<SimulationEventWorldEntity>) {
            self.events
                .extend(evs.drain(..).map(SimulationWorldEvent::Entity));
        }

        pub fn take(&mut self) -> SimulationWorldEvents {
            let events = self.pool.new();
            std::mem::replace(&mut self.events, events)
        }

        pub fn for_each<F>(&self, mut f: F)
        where
            for<'a> F: HiFnMut<&'a SimulationWorldEvent, ()>,
        {
            self.events.iter().for_each(move |ev| f.call_mut(ev))
        }
    }

    /// simulation events are events that should be
    /// handled by a upper component
    /// it's also useful to cleanly split prediction code
    /// from actual ticks.. prediction code can simply ignore
    /// these events
    #[hiarc_safer_rc_refcell]
    #[derive(Debug, Hiarc)]
    pub struct SimulationEvents {
        events: PoolLinkedHashMap<GameEntityId, SimulationWorldEvents>,
        pool: Pool<LinkedHashMap<GameEntityId, SimulationWorldEvents>>,
    }

    #[hiarc_safer_rc_refcell]
    impl Default for SimulationEvents {
        fn default() -> Self {
            Self::new()
        }
    }

    #[hiarc_safer_rc_refcell]
    impl SimulationEvents {
        pub fn new() -> Self {
            let pool = Pool::with_capacity(2);
            Self {
                events: pool.new(),
                pool,
            }
        }

        pub fn insert_world_evs(&mut self, stage_id: GameEntityId, ev: SimulationWorldEvents) {
            self.events.insert(stage_id, ev);
        }

        pub fn take(&mut self) -> PoolLinkedHashMap<GameEntityId, SimulationWorldEvents> {
            let events = self.pool.new();
            std::mem::replace(&mut self.events, events)
        }

        pub fn for_each<F>(&self, mut f: F)
        where
            for<'a> F: HiFnMut<&'a SimulationWorldEvents, ()>,
        {
            self.events.values().for_each(move |ev| f.call_mut(ev))
        }
    }

    pub struct SimulationPipe<'a> {
        pub collision: &'a Collision,
    }

    impl<'a> SimulationPipe<'a> {
        pub fn new(collision: &'a Collision) -> Self {
            Self { collision }
        }
    }

    pub struct SimulationPipeStage<'a> {
        // should only be true inside a client's simulation pipe
        pub is_prediction: bool,

        pub collision: &'a Collision,

        pub stage_id: &'a GameEntityId,

        pub world_pool: &'a mut WorldPool,
    }

    impl<'a> SimulationPipeStage<'a> {
        pub fn new(
            is_prediction: bool,
            collision: &'a Collision,
            stage_id: &'a GameEntityId,
            world_pool: &'a mut WorldPool,
        ) -> Self {
            Self {
                is_prediction,
                collision,
                stage_id,
                world_pool,
            }
        }
    }

    pub trait SimulationPipeCharactersGetter {
        fn for_other_characters_in_range(
            &mut self,
            char_pos: &vec2,
            radius: f32,
            for_each_func: &mut dyn FnMut(&mut Character, &mut LinkedHashSet<GameEntityId>),
        );
        fn get_other_character_id_and_cores_iter_by_ids_mut(
            &mut self,
            ids: &[GameEntityId],
            for_each_func: &mut dyn FnMut(
                &GameEntityId,
                &mut Core,
                &mut CoreReusable,
                &mut CharacterPos,
            ) -> ControlFlow<()>,
        ) -> ControlFlow<()>;
        fn get_other_character_pos_by_id(&self, other_char_id: &GameEntityId) -> &vec2;
        fn get_other_character_by_id_mut(&mut self, other_char_id: &GameEntityId)
            -> &mut Character;
        fn kill_character(&mut self, char_id: &GameEntityId);
    }

    pub struct SimulationPipeCharacter<'a> {
        pub characters: &'a mut dyn SimulationPipeCharactersGetter,

        pub collision: &'a Collision,
    }

    impl<'a> SimulationPipeCharacter<'a> {
        pub fn new(
            characters: &'a mut dyn SimulationPipeCharactersGetter,
            collision: &'a Collision,
        ) -> Self {
            Self {
                characters,
                collision,
            }
        }
    }

    pub struct SimulationPipeCharacters<'a> {
        pub characters: &'a mut Characters,
        pub owner_character: GameEntityId,
    }

    impl<'a> SimulationPipeCharacters<'a> {
        pub fn get_characters_except_owner(
            &mut self,
        ) -> CharactersView<impl Fn(&GameEntityId) -> bool + '_> {
            CharactersView::new(self.characters, |id| *id != self.owner_character)
        }
        pub fn get_characters(&mut self) -> CharactersView<impl Fn(&GameEntityId) -> bool> {
            CharactersView::new(self.characters, |_| true)
        }
        pub fn get_owner_character_view(
            &mut self,
        ) -> CharactersView<impl Fn(&GameEntityId) -> bool + '_> {
            CharactersView::new(self.characters, |id| *id == self.owner_character)
        }
        pub fn get_owner_character(&mut self) -> &mut Character {
            self.characters.get_mut(&self.owner_character).unwrap()
        }
    }

    pub struct SimulationPipeProjectile<'a> {
        pub collision: &'a Collision,

        pub characters_helper: SimulationPipeCharacters<'a>,
        pub field: &'a CharacterPositionPlayfield,
    }

    impl<'a> SimulationPipeProjectile<'a> {
        pub fn new(
            collision: &'a Collision,
            characters: &'a mut Characters,
            owner_character: GameEntityId,
            field: &'a CharacterPositionPlayfield,
        ) -> Self {
            Self {
                collision,
                characters_helper: SimulationPipeCharacters {
                    characters,
                    owner_character,
                },
                field,
            }
        }
    }

    pub struct SimulationPipePickup<'a> {
        pub characters: &'a mut Characters,
        pub field: &'a CharacterPositionPlayfield,
    }

    impl<'a> SimulationPipePickup<'a> {
        pub fn new(characters: &'a mut Characters, field: &'a CharacterPositionPlayfield) -> Self {
            Self { characters, field }
        }
    }

    pub struct SimulationPipeFlag<'a> {
        pub collision: &'a Collision,

        pub characters: &'a mut Characters,
        pub field: &'a CharacterPositionPlayfield,

        pub is_prediction: bool,
    }

    impl<'a> SimulationPipeFlag<'a> {
        pub fn new(
            collision: &'a Collision,
            characters: &'a mut Characters,
            field: &'a CharacterPositionPlayfield,
            is_prediction: bool,
        ) -> Self {
            Self {
                collision,
                characters,
                field,
                is_prediction,
            }
        }
    }

    pub struct SimulationPipeLaser<'a> {
        pub collision: &'a Collision,

        pub characters_helper: SimulationPipeCharacters<'a>,
        pub field: &'a CharacterPositionPlayfield,
    }

    impl<'a> SimulationPipeLaser<'a> {
        pub fn new(
            collision: &'a Collision,
            characters: &'a mut Characters,
            owner_character: GameEntityId,
            field: &'a CharacterPositionPlayfield,
        ) -> Self {
            Self {
                collision,
                characters_helper: SimulationPipeCharacters {
                    characters,
                    owner_character,
                },
                field,
            }
        }
    }
}
