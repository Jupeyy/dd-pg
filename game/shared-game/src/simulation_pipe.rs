pub mod simulation_pipe {
    use std::marker::PhantomData;
    use std::ops::{ControlFlow, Deref};

    use game_interface::events::{EventIdGenerator, GameWorldEvents, GameWorldNotificationEvent};
    use game_interface::pooling::GamePooling;
    use game_interface::types::id_types::{
        CharacterId, CtfFlagId, LaserId, PickupId, ProjectileId, StageId,
    };
    use hashlink::LinkedHashMap;
    use hiarc::{hi_closure, HiFnMut};
    use hiarc::{hiarc_safer_rc_refcell, Hiarc};
    use math::math::vector::vec2;
    use serde::{Deserialize, Serialize};

    use crate::entities::character::character::{CharactersView, RemovedCharacters};
    use crate::entities::character::core::character_core::{Core, CoreReusable};
    use crate::entities::character::pos::character_pos::{
        CharacterPos, CharacterPositionPlayfield,
    };
    use crate::entities::flag::flag::Flags;
    use crate::events::events::{
        CharacterTickEvent, FlagEvent, LaserEvent, PickupEvent, ProjectileEvent,
    };
    use crate::world::world::GameObjectsWorld;
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
            id: ProjectileId,
            ev: ProjectileEvent,
        },
        Pickup {
            id: PickupId,
            ev: PickupEvent,
        },
        Flag {
            id: CtfFlagId,
            ev: FlagEvent,
        },
        Laser {
            id: LaserId,
            ev: LaserEvent,
        },
    }

    #[derive(Debug, Hiarc, Serialize, Deserialize)]
    pub struct SimulationEventWorldEntity {
        pub owner_id: Option<CharacterId>,
        pub ev: SimulationEventWorldEntityType,
    }

    #[derive(Debug, Hiarc, Serialize, Deserialize)]
    pub enum SimulationWorldEvent {
        Entity(SimulationEventWorldEntity),
        Notification(GameWorldNotificationEvent),
    }

    #[hiarc_safer_rc_refcell]
    #[derive(Debug, Default, Hiarc)]
    pub struct SimulationWorldEvents {
        evs: Vec<SimulationWorldEvent>,

        _p: PhantomData<GameObjectsWorld>,
        _g: PhantomData<GamePooling>,
        _e: PhantomData<EventIdGenerator>,
        _s: PhantomData<pool::mt_datatypes::PoolLinkedHashMap<StageId, GameWorldEvents>>,
    }

    #[hiarc_safer_rc_refcell]
    impl SimulationWorldEvents {
        pub fn push(&mut self, ev: SimulationWorldEvent) {
            self.evs.push(ev);
        }

        pub fn push_world(
            &mut self,
            owner_id: Option<CharacterId>,
            ev: SimulationEventWorldEntityType,
        ) {
            self.evs
                .push(SimulationWorldEvent::Entity(SimulationEventWorldEntity {
                    ev,
                    owner_id,
                }));
        }

        pub fn take(&mut self) -> Vec<SimulationWorldEvent> {
            std::mem::take(&mut self.evs)
        }

        pub fn set(&mut self, evs: Vec<SimulationWorldEvent>) {
            self.evs = evs;
        }

        pub fn for_each<F>(&self, mut f: F)
        where
            for<'a> F: HiFnMut<&'a SimulationWorldEvent, ()>,
        {
            self.evs.iter().for_each(move |ev| f.call_mut(ev))
        }

        pub fn for_each_evs<F>(&self, mut f: F)
        where
            for<'a> F: HiFnMut<&'a Vec<SimulationWorldEvent>, ()>,
        {
            f.call_mut(&self.evs);
        }
    }

    #[hiarc_safer_rc_refcell]
    #[derive(Debug, Hiarc)]
    pub struct SimulationStageEvents {
        events: SimulationWorldEvents,

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
            Self {
                events: Default::default(),

                _py: Default::default(),
            }
        }

        pub fn push(&mut self, ev: SimulationWorldEvent) {
            self.events.push(ev);
        }

        pub fn take(&self) -> Vec<SimulationWorldEvent> {
            self.events.take()
        }

        pub fn clone_evs(&self) -> SimulationWorldEvents {
            self.events.clone()
        }

        pub fn for_each<F>(&self, mut f: F)
        where
            for<'a> F: HiFnMut<&'a SimulationWorldEvent, ()>,
        {
            let evs = self.events.take();
            evs.iter().for_each(|ev| f.call_mut(ev));
            self.events.set(evs);
        }

        pub fn for_each_evs<F>(&self, f: F)
        where
            for<'a> F: HiFnMut<&'a Vec<SimulationWorldEvent>, ()>,
        {
            self.events.for_each_evs(f)
        }
    }

    #[hiarc_safer_rc_refcell]
    #[derive(Debug, Default, Hiarc)]
    pub struct SimulationEventsInner(LinkedHashMap<StageId, SimulationStageEvents>);

    #[hiarc_safer_rc_refcell]
    impl SimulationEventsInner {
        pub fn remove(&mut self, id: &StageId) {
            self.0.remove(id);
        }
        pub fn insert(&mut self, id: StageId, evs: SimulationStageEvents) {
            self.0.insert(id, evs);
        }

        pub fn clear_events(&mut self) {
            for stage_evs in self.0.values_mut() {
                stage_evs.take();
            }
        }

        #[inline]
        pub fn for_each<F>(&self, mut f: F)
        where
            for<'a> F: HiFnMut<(&'a StageId, &'a Vec<SimulationWorldEvent>), ()>,
        {
            self.0.iter().for_each(move |(id, ev)| {
                let f = &mut f;
                ev.for_each_evs(hi_closure!(
                    <F: for<'b> HiFnMut<(&'b StageId, &'b Vec<SimulationWorldEvent>), ()>>,
                    [f: &mut F, id: &StageId],
                    |evs: &Vec<SimulationWorldEvent>| -> () {
                        f.call_mut((id, evs));
                    }
                ));
            })
        }
    }

    #[derive(Debug, Hiarc)]
    pub struct SimulationStageEventsRaii {
        stage_id: StageId,
        events: SimulationStageEvents,

        events_container: SimulationEventsInner,
    }

    impl SimulationStageEventsRaii {
        pub fn new(events_container: SimulationEventsInner, stage_id: StageId) -> Self {
            let events = SimulationStageEvents::default();
            events_container.insert(stage_id, events.clone());
            Self {
                stage_id,
                events,
                events_container,
            }
        }
    }

    impl Deref for SimulationStageEventsRaii {
        type Target = SimulationStageEvents;

        fn deref(&self) -> &Self::Target {
            &self.events
        }
    }

    impl Drop for SimulationStageEventsRaii {
        fn drop(&mut self) {
            self.events_container.remove(&self.stage_id);
        }
    }

    /// simulation events are events that should be
    /// handled by a upper component
    /// it's also useful to cleanly split prediction code
    /// from actual ticks.. prediction code can simply ignore
    /// these events
    #[derive(Debug, Default, Hiarc)]
    pub struct SimulationEvents {
        events: SimulationEventsInner,
    }

    impl SimulationEvents {
        pub fn init_stage(&mut self, stage_id: StageId) -> SimulationStageEventsRaii {
            let events_container = self.events.clone();
            SimulationStageEventsRaii::new(events_container, stage_id)
        }
    }

    impl Deref for SimulationEvents {
        type Target = SimulationEventsInner;

        fn deref(&self) -> &Self::Target {
            &self.events
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

        pub stage_id: &'a StageId,

        pub world_pool: &'a WorldPool,
    }

    impl<'a> SimulationPipeStage<'a> {
        pub fn new(
            is_prediction: bool,
            collision: &'a Collision,
            stage_id: &'a StageId,
            world_pool: &'a WorldPool,
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
        fn for_other_characters_in_range_mut(
            &mut self,
            char_pos: &vec2,
            radius: f32,
            for_each_func: &mut dyn FnMut(&mut Character, &RemovedCharacters),
        );
        fn get_other_character_id_and_cores_iter_by_ids_mut(
            &mut self,
            ids: &[CharacterId],
            for_each_func: &mut dyn FnMut(
                &CharacterId,
                &mut Core,
                &mut CoreReusable,
                &mut CharacterPos,
            ) -> ControlFlow<()>,
        ) -> ControlFlow<()>;
        fn get_other_character_pos_by_id(&self, other_char_id: &CharacterId) -> &vec2;
        fn get_other_character_by_id_mut(&mut self, other_char_id: &CharacterId) -> &mut Character;
        fn kill_character(&mut self, char_id: &CharacterId);
    }

    pub struct SimulationPipeCharacter<'a> {
        pub characters: &'a mut dyn SimulationPipeCharactersGetter,
        pub entity_events: &'a mut Vec<CharacterTickEvent>,

        pub collision: &'a Collision,
    }

    impl<'a> SimulationPipeCharacter<'a> {
        pub fn new(
            characters: &'a mut dyn SimulationPipeCharactersGetter,
            entity_events: &'a mut Vec<CharacterTickEvent>,
            collision: &'a Collision,
        ) -> Self {
            Self {
                characters,
                entity_events,
                collision,
            }
        }
    }

    pub struct SimulationPipeCharacters<'a> {
        pub characters: &'a mut Characters,
        pub owner_character: CharacterId,
    }

    impl SimulationPipeCharacters<'_> {
        pub fn get_characters_except_owner(
            &mut self,
        ) -> CharactersView<impl Fn(&CharacterId) -> bool + '_> {
            CharactersView::new(self.characters, |id| *id != self.owner_character)
        }
        pub fn get_characters(&mut self) -> CharactersView<impl Fn(&CharacterId) -> bool> {
            CharactersView::new(self.characters, |_| true)
        }
        pub fn get_owner_character_view(
            &mut self,
        ) -> CharactersView<impl Fn(&CharacterId) -> bool + '_> {
            CharactersView::new(self.characters, |id| *id == self.owner_character)
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
            owner_character: CharacterId,
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

        pub other_team_flags: &'a Flags,

        pub is_prediction: bool,
    }

    impl<'a> SimulationPipeFlag<'a> {
        pub fn new(
            collision: &'a Collision,
            characters: &'a mut Characters,
            field: &'a CharacterPositionPlayfield,
            other_team_flags: &'a Flags,
            is_prediction: bool,
        ) -> Self {
            Self {
                collision,
                characters,
                field,
                is_prediction,
                other_team_flags,
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
            owner_character: CharacterId,
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
