pub mod character_pos {
    use std::{
        collections::{hash_map::Entry, HashMap, HashSet},
        num::NonZeroU16,
    };

    use game_interface::types::game::GameEntityId;
    use hashlink::LinkedHashMap;
    use hiarc::{hiarc_safer_rc_refcell, Hiarc};
    use math::math::{
        round_to_int,
        vector::{usvec2, vec2},
    };
    use pool::{datatypes::PoolHashSet, pool::Pool};

    /// The playfield of characters.
    /// For simplicity characters that are outside of the map
    /// are clamped to the closest map position.
    /// in width and height.
    /// Works by radius only.
    /// Distance check should still additionally be done.
    #[hiarc_safer_rc_refcell]
    #[derive(Debug, Hiarc)]
    pub struct CharacterPositionPlayfield {
        // crates like tinyset seem to be buggy, we want
        // this field to be as small as possible
        world: HashMap<usize, HashSet<GameEntityId>>,
        width: NonZeroU16,
        height: NonZeroU16,

        entities: LinkedHashMap<GameEntityId, usvec2>,

        pool: Pool<HashSet<GameEntityId>>,
    }

    #[hiarc_safer_rc_refcell]
    impl CharacterPositionPlayfield {
        pub fn new(width: NonZeroU16, height: NonZeroU16) -> Self {
            Self {
                world: Default::default(),
                width,
                height,

                entities: Default::default(),

                pool: Pool::with_capacity(2),
            }
        }

        pub fn width(&self) -> NonZeroU16 {
            self.width
        }
        pub fn height(&self) -> NonZeroU16 {
            self.height
        }

        fn get_at_mut(&mut self, pos: &usvec2) -> Entry<'_, usize, HashSet<GameEntityId>> {
            let index = pos.y as usize * self.width.get() as usize + pos.x as usize;

            self.world.entry(index)
        }

        pub fn add_or_move(&mut self, id: GameEntityId, pos: vec2) {
            let nx = (round_to_int(pos.x) / 32).clamp(0, self.width.get() as i32 - 1);
            let ny = (round_to_int(pos.y) / 32).clamp(0, self.height.get() as i32 - 1);
            let pos = usvec2::new(nx as u16, ny as u16);

            let entry = self.entities.entry(id).or_insert_with(|| pos);
            let old_pos = *entry;
            *entry = pos;
            let cur_ids = self.get_at_mut(&old_pos);
            if let Entry::Occupied(mut ids) = cur_ids {
                ids.get_mut().remove(&id);
                ids.get_mut().shrink_to_fit();
                if ids.get().is_empty() {
                    ids.remove();
                }
            }
            let new_ids = self.get_at_mut(&pos).or_default();
            new_ids.insert(id);
        }

        pub fn remove(&mut self, id: GameEntityId) {
            if let Some(entity) = self.entities.remove(&id) {
                let cur_ids = self.get_at_mut(&entity);

                if let Entry::Occupied(mut ids) = cur_ids {
                    ids.get_mut().remove(&id);
                    ids.get_mut().shrink_to_fit();
                    if ids.get().is_empty() {
                        ids.remove();
                    }
                }
            }
        }

        pub fn by_radius(&self, pos: &vec2, radius: f32) -> PoolHashSet<GameEntityId> {
            let min_x =
                (round_to_int(pos.x - radius) / 32).clamp(0, self.width.get() as i32 - 1) as u16;
            let min_y =
                (round_to_int(pos.y - radius) / 32).clamp(0, self.height.get() as i32 - 1) as u16;
            let max_x =
                (round_to_int(pos.x + radius) / 32).clamp(0, self.width.get() as i32 - 1) as u16;
            let max_y =
                (round_to_int(pos.y + radius) / 32).clamp(0, self.height.get() as i32 - 1) as u16;

            let mut res = self.pool.new();
            for x in min_x..=max_x {
                for y in min_y..=max_y {
                    let index = y as usize * self.width.get() as usize + x as usize;

                    if let Some(ids) = self.world.get(&index) {
                        res.extend(ids.iter());
                    }
                }
            }
            res
        }
    }

    impl CharacterPositionPlayfield {
        pub fn get_character_pos(&self, pos: vec2, id: GameEntityId) -> CharacterPos {
            self.add_or_move(id, pos);

            CharacterPos {
                pos,
                field: self.clone(),
                id,
            }
        }
    }

    #[derive(Debug, Hiarc)]
    pub struct CharacterPos {
        pos: vec2,
        pub field: CharacterPositionPlayfield,
        id: GameEntityId,
    }

    impl CharacterPos {
        pub fn pos(&self) -> &vec2 {
            &self.pos
        }

        pub fn move_pos(&mut self, pos: vec2) {
            self.field.add_or_move(self.id, pos);
            self.pos = pos;
        }

        pub fn quantinize(&mut self) {
            self.pos.x = round_to_int(self.pos.x) as f32;
            self.pos.y = round_to_int(self.pos.y) as f32;
        }

        pub fn in_range(&self, radius: f32) -> PoolHashSet<GameEntityId> {
            self.field.by_radius(&self.pos, radius)
        }
    }

    impl Drop for CharacterPos {
        fn drop(&mut self) {
            self.field.remove(self.id);
        }
    }
}
