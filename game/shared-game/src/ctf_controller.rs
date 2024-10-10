pub mod ctf_controller {
    use game_interface::events::GameFlagEventSound;
    use hiarc::Hiarc;
    use math::math::distance;

    use crate::{
        entities::{
            character::core::character_core,
            flag::flag::{Flag, Flags},
        },
        events::events::FlagEvent,
        simulation_pipe::simulation_pipe::{
            SimulationEntityEvents, SimulationEventWorldEntityType,
        },
        world::world::GameWorld,
    };

    #[derive(Debug, Hiarc, Default)]
    pub struct CtfController {}

    impl CtfController {
        fn check_flags(events: &SimulationEntityEvents, flags: &mut Flags, other_flags: &Flags) {
            for (_, flag) in flags.iter_mut() {
                if let Some(carrier) = flag.core.carrier {
                    for other_flag in other_flags.values() {
                        if other_flag.core.carrier.is_none()
                            && other_flag.core.pos == other_flag.core.spawn_pos
                            && distance(&flag.core.pos, &other_flag.core.pos)
                                < Flag::PHYSICAL_SIZE + character_core::PHYSICAL_SIZE
                        {
                            let flag_pos = flag.core.pos;
                            flag.reset(false);
                            events.push(
                                Some(carrier),
                                SimulationEventWorldEntityType::Flag {
                                    id: flag.base.game_element_id,
                                    ev: FlagEvent::Capture { pos: flag_pos },
                                },
                            );
                            events.push(
                                Some(carrier),
                                SimulationEventWorldEntityType::Flag {
                                    id: flag.base.game_element_id,
                                    ev: FlagEvent::Sound {
                                        pos: flag_pos / 32.0,
                                        ev: GameFlagEventSound::Capture,
                                    },
                                },
                            );
                            flag.core.non_linear_event += 1;
                        }
                    }
                }
            }
        }

        pub fn tick(world: &mut GameWorld) {
            Self::check_flags(
                &world.simulation_events,
                &mut world.red_flags,
                &world.blue_flags,
            );
            Self::check_flags(
                &world.simulation_events,
                &mut world.blue_flags,
                &world.red_flags,
            );
        }
    }
}
