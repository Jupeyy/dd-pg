pub mod character_hook {
    use game_interface::types::game::GameEntityId;
    use hashlink::{LinkedHashMap, LinkedHashSet};
    use hiarc::{hiarc_safer_rc_refcell, Hiarc};
    use math::math::{round_to_int, vector::vec2};
    use num_derive::FromPrimitive;
    use serde::{Deserialize, Serialize};

    #[derive(
        Debug,
        Hiarc,
        Clone,
        Copy,
        Serialize,
        Deserialize,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        FromPrimitive,
    )]
    pub enum HookState {
        /// the hook is did not attach to anything and goes into the character again
        RetractStart,
        /// like [HookState::RetractStart] but one tick later
        RetractEnd,
        /// the hook is currently flying
        HookFlying,
        /// the hook is currently attached to something (player, ground or whatever)
        HookGrabbed,
    }

    #[derive(Debug, Hiarc, Copy, Clone, Default, Serialize, Deserialize)]
    pub enum Hook {
        #[default]
        None,
        Active {
            hook_pos: vec2,
            hook_dir: vec2,
            hook_tele_base: vec2,
            hook_tick: i32,
            hook_state: HookState,
        },
        WaitsForRelease,
    }

    #[derive(Debug, Hiarc, Default)]
    struct HookCharacter {
        hooked_char: Option<GameEntityId>,
        hooked_by: LinkedHashSet<GameEntityId>,

        hook: Hook,
    }

    /// all characters' hooking relation to each other
    #[hiarc_safer_rc_refcell]
    #[derive(Debug, Hiarc, Default)]
    pub struct HookedCharacters {
        characters: LinkedHashMap<GameEntityId, HookCharacter>,
    }

    #[hiarc_safer_rc_refcell]
    impl HookedCharacters {
        pub fn add_or_set(
            &mut self,
            id: GameEntityId,
            hook: Hook,
            mut hooked_char: Option<GameEntityId>,
        ) {
            if hooked_char.is_some_and(|hooked_char| !self.characters.contains_key(&hooked_char)) {
                hooked_char = None;
            }

            let entry = self
                .characters
                .entry(id)
                .or_insert(HookCharacter::default());

            let prev_hooked_char = entry.hooked_char;

            entry.hook = hook;
            entry.hooked_char = hooked_char;

            if let Some(hooked_char) = prev_hooked_char {
                self.characters
                    .get_mut(&hooked_char)
                    .unwrap()
                    .hooked_by
                    .remove(&id);
            }
            if let Some(hooked_char) = hooked_char {
                if let Some(character) = self.characters.get_mut(&hooked_char) {
                    character.hooked_by.insert(id);
                }
            }
        }

        pub fn remove(&mut self, id: &GameEntityId) {
            let char = self.characters.remove(id).unwrap();

            for hooked_by in char.hooked_by {
                let hooking_char = self.characters.get_mut(&hooked_by).unwrap();
                hooking_char.hooked_char = None;
                if let Hook::Active { .. } = hooking_char.hook {
                    hooking_char.hook = Hook::WaitsForRelease;
                }
            }

            if let Some(hooked_char) = char.hooked_char {
                self.characters
                    .get_mut(&hooked_char)
                    .unwrap()
                    .hooked_by
                    .remove(&hooked_char);
            }
        }

        pub(super) fn get_hook(&self, id: &GameEntityId) -> (Hook, Option<GameEntityId>) {
            let char = self.characters.get(id).unwrap();
            (char.hook, char.hooked_char)
        }
    }

    impl HookedCharacters {
        pub fn get_new_hook(&self, id: GameEntityId) -> CharacterHook {
            self.add_or_set(id, Default::default(), Default::default());
            CharacterHook {
                hooked_players: self.clone(),
                id,
            }
        }
    }

    #[derive(Debug, Hiarc)]
    pub struct CharacterHook {
        id: GameEntityId,
        hooked_players: HookedCharacters,
    }

    impl CharacterHook {
        pub fn hook(&self) -> Hook {
            self.hooked_players.get_hook(&self.id).0
        }
        pub fn hooked_char(&self) -> Option<GameEntityId> {
            self.hooked_players.get_hook(&self.id).1
        }

        pub fn get(&self) -> (Hook, Option<GameEntityId>) {
            self.hooked_players.get_hook(&self.id)
        }

        pub fn set(&mut self, hook: Hook, hooked_char: Option<GameEntityId>) {
            self.hooked_players.add_or_set(self.id, hook, hooked_char)
        }

        pub fn quantinize(&mut self) {
            let (mut hook, hooked_char) = self.get();
            if let Hook::Active {
                hook_pos, hook_dir, ..
            } = &mut hook
            {
                hook_pos.x = round_to_int(hook_pos.x) as f32;
                hook_pos.y = round_to_int(hook_pos.y) as f32;
                hook_dir.x = round_to_int(hook_dir.x * 256.0) as f32 / 256.0;
                hook_dir.y = round_to_int(hook_dir.y * 256.0) as f32 / 256.0;
            }
            self.set(hook, hooked_char);
        }
    }

    impl Drop for CharacterHook {
        fn drop(&mut self) {
            self.hooked_players.remove(&self.id);
        }
    }
}
