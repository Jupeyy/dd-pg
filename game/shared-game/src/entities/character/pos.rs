pub mod character_pos {
    use std::{collections::hash_map::Entry, num::NonZeroU16, ops::Deref};

    use game_interface::types::game::GameEntityId;
    use hiarc::{hiarc_safer_rc_refcell, Hiarc};
    use math::math::{
        round_to_int,
        vector::{usvec2, vec2},
    };
    use pool::{
        datatypes::{PoolBox, PoolFxHashSet, PoolVec},
        pool::Pool,
    };
    use rustc_hash::{FxHashMap, FxHashSet};

    // Vec seems to perform better than BTreeSet.
    type FieldEntitiesList = Vec<GameEntityId>;
    type PoolFieldEntitiesList = PoolVec<GameEntityId>;

    type FieldEntitiesListPool = Pool<Vec<GameEntityId>>;
    type BoxFieldEntitiesListPool = Pool<Box<Vec<GameEntityId>>>;

    #[derive(Debug, Hiarc)]
    enum WorldMap {
        Hashed {
            map: FxHashMap<u32, PoolFieldEntitiesList>,
            world_pool: FieldEntitiesListPool,
        },
        Grid {
            map: Vec<Option<PoolBox<FieldEntitiesList>>>,
            world_pool: BoxFieldEntitiesListPool,
        },
    }

    impl WorldMap {
        fn new(width: NonZeroU16, height: NonZeroU16) -> Self {
            // if size smaller than a certain threshold fallback to faster
            // grid based approach, works for small maps, which is common
            // for vanilla.
            let size = width.get() as usize * height.get() as usize;
            if size * 8 < 131072 {
                Self::Grid {
                    map: {
                        let mut res = Vec::with_capacity(size);
                        res.resize_with(size, || None);
                        res
                    },
                    world_pool: Pool::with_capacity(64),
                }
            } else {
                Self::Hashed {
                    map: Default::default(),
                    world_pool: Pool::with_capacity(64),
                }
            }
        }

        fn entry(&mut self, index: u32) -> WorldMapEntry {
            match self {
                WorldMap::Hashed { map, world_pool } => WorldMapEntry::Hashed {
                    entry: map.entry(index),
                    pool: world_pool,
                },
                WorldMap::Grid { map, world_pool } => WorldMapEntry::Grid {
                    entry: &mut map[index as usize],
                    pool: world_pool,
                },
            }
        }

        fn get(&self, index: u32) -> Option<&FieldEntitiesList> {
            match self {
                WorldMap::Hashed { map, .. } => map.get(&index).map(|e| e.deref()),
                WorldMap::Grid { map, .. } => map
                    .get(index as usize)
                    .and_then(|e| e.as_ref().map(|e| (*e).as_ref())),
            }
        }
    }

    enum WorldMapEntry<'a> {
        Hashed {
            entry: Entry<'a, u32, PoolFieldEntitiesList>,
            pool: &'a FieldEntitiesListPool,
        },
        Grid {
            entry: &'a mut Option<PoolBox<FieldEntitiesList>>,
            pool: &'a BoxFieldEntitiesListPool,
        },
    }

    fn sorted_insert<T: Ord>(vec: &mut Vec<T>, item: T) {
        let pos = vec.binary_search(&item).unwrap_or_else(|e| e);
        vec.insert(pos, item);
    }
    fn remove_item<T: Ord + PartialEq>(vec: &mut Vec<T>, item: &T) -> bool {
        if let Ok(pos) = vec.binary_search(item) {
            vec.remove(pos);
            true
        } else {
            false
        }
    }
    impl<'a> WorldMapEntry<'a> {
        fn try_remove(self, id: &GameEntityId) {
            match self {
                WorldMapEntry::Hashed { entry, .. } => {
                    if let Entry::Occupied(mut ids) = entry {
                        remove_item(ids.get_mut(), id);
                        if ids.get().is_empty() {
                            ids.remove();
                        }
                    }
                }
                WorldMapEntry::Grid { entry, .. } => {
                    if let Some(ids) = entry {
                        remove_item(ids, id);
                        if ids.is_empty() {
                            *entry = None;
                        }
                    }
                }
            }
        }
        fn insert(self, id: GameEntityId) {
            match self {
                WorldMapEntry::Hashed { entry, pool } => {
                    let ids = entry.or_insert_with(|| pool.new());
                    sorted_insert(ids, id);
                }
                WorldMapEntry::Grid { entry, pool } => {
                    let ids = entry.get_or_insert_with(|| pool.new());
                    sorted_insert(ids, id);
                }
            }
        }
    }

    /// The playfield of characters.
    ///
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
        world: WorldMap,
        width: NonZeroU16,
        height: NonZeroU16,

        entities: FxHashMap<GameEntityId, usvec2>,

        pool: Pool<FxHashSet<GameEntityId>>,
        vec_pool: Pool<Vec<GameEntityId>>,
    }

    #[hiarc_safer_rc_refcell]
    impl CharacterPositionPlayfield {
        pub fn new(width: NonZeroU16, height: NonZeroU16) -> Self {
            Self {
                world: WorldMap::new(width, height),
                width,
                height,

                entities: Default::default(),

                pool: Pool::with_capacity(2),
                vec_pool: Pool::with_capacity(2),
            }
        }

        pub fn width(&self) -> NonZeroU16 {
            self.width
        }
        pub fn height(&self) -> NonZeroU16 {
            self.height
        }

        #[inline]
        fn index_at(&self, pos: &usvec2) -> u32 {
            pos.y as u32 * self.width.get() as u32 + pos.x as u32
        }

        #[inline]
        fn get_at_index_mut(world: &mut WorldMap, index: u32) -> WorldMapEntry {
            world.entry(index)
        }

        #[inline]
        fn get_at_mut(&mut self, pos: &usvec2) -> WorldMapEntry {
            let index = self.index_at(pos);
            Self::get_at_index_mut(&mut self.world, index)
        }

        /// Returns true, if the new position is in the same field as the previous one.
        /// A.k.a. a call to `by_radius` would give the same result again.
        pub fn add_or_move(&mut self, id: GameEntityId, pos: vec2) -> bool {
            let nx = (round_to_int(pos.x) / 32).clamp(0, self.width.get() as i32 - 1);
            let ny = (round_to_int(pos.y) / 32).clamp(0, self.height.get() as i32 - 1);
            let pos = usvec2::new(nx as u16, ny as u16);

            let entry = self.entities.entry(id);
            let is_new = matches!(entry, Entry::Vacant(_));
            let entry = entry.or_insert_with(|| pos);
            let old_pos = *entry;
            *entry = pos;

            let old_index = self.index_at(&old_pos);
            let new_index = self.index_at(&pos);

            if old_index != new_index || is_new {
                let cur_ids = Self::get_at_index_mut(&mut self.world, old_index);
                cur_ids.try_remove(&id);
                let new_ids = Self::get_at_index_mut(&mut self.world, new_index);
                new_ids.insert(id);
                false
            } else {
                true
            }
        }

        pub fn remove(&mut self, id: GameEntityId) {
            if let Some(entity) = self.entities.remove(&id) {
                let cur_ids = self.get_at_mut(&entity);
                cur_ids.try_remove(&id);
            }
        }

        #[inline]
        fn by_min_max_impl(
            &self,
            min_x: u32,
            min_y: u32,
            max_x: u32,
            max_y: u32,
            mut add: impl FnMut(&mut dyn Iterator<Item = &GameEntityId>),
        ) {
            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    let index = y * self.width.get() as u32 + x;

                    if let Some(ids) = self.world.get(index) {
                        add(&mut ids.iter());
                    }
                }
            }
        }

        #[inline]
        fn by_radiusf_impl(
            &self,
            pos: &vec2,
            radius: f32,
            add: impl FnMut(&mut dyn Iterator<Item = &GameEntityId>),
        ) {
            let min_x =
                (round_to_int(pos.x - radius) / 32).clamp(0, self.width.get() as i32 - 1) as u32;
            let min_y =
                (round_to_int(pos.y - radius) / 32).clamp(0, self.height.get() as i32 - 1) as u32;
            let max_x =
                (round_to_int(pos.x + radius) / 32).clamp(0, self.width.get() as i32 - 1) as u32;
            let max_y =
                (round_to_int(pos.y + radius) / 32).clamp(0, self.height.get() as i32 - 1) as u32;

            self.by_min_max_impl(min_x, min_y, max_x, max_y, add)
        }

        #[inline]
        fn by_radius_impl(
            &self,
            pos: &vec2,
            radius: i32,
            add: impl FnMut(&mut dyn Iterator<Item = &GameEntityId>),
        ) {
            let x = round_to_int(pos.x);
            let y = round_to_int(pos.y);
            let min_x = ((x - radius) / 32).clamp(0, self.width.get() as i32 - 1) as u32;
            let min_y = ((y - radius) / 32).clamp(0, self.height.get() as i32 - 1) as u32;
            let max_x = ((x + radius) / 32).clamp(0, self.width.get() as i32 - 1) as u32;
            let max_y = ((y + radius) / 32).clamp(0, self.height.get() as i32 - 1) as u32;

            self.by_min_max_impl(min_x, min_y, max_x, max_y, add)
        }

        pub fn by_radiusf(&self, pos: &vec2, radius: f32) -> PoolVec<GameEntityId> {
            let mut res = self.vec_pool.new();
            self.by_radiusf_impl(pos, radius, |ids| res.extend(ids));
            res
        }

        /// Generally faster than `by_radiusf`
        pub fn by_radius(&self, pos: &vec2, radius: i32) -> PoolVec<GameEntityId> {
            let mut res = self.vec_pool.new();
            self.by_radius_impl(pos, radius, |ids| res.extend(ids));
            res
        }

        /// The returned set is not sorted in any way
        pub fn by_radius_set(&self, pos: &vec2, radius: i32) -> PoolFxHashSet<GameEntityId> {
            let mut res = self.pool.new();
            self.by_radius_impl(pos, radius, |ids| res.extend(ids));
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

        /// Returns true if the position field changed,
        /// and thus `in_range` would return different ids.
        pub fn move_pos(&mut self, pos: vec2) -> bool {
            let has_moved = self.field.add_or_move(self.id, pos);
            self.pos = pos;
            has_moved
        }

        pub fn quantinize(&mut self) {
            self.pos.x = round_to_int(self.pos.x) as f32;
            self.pos.y = round_to_int(self.pos.y) as f32;
            self.field.add_or_move(self.id, self.pos);
        }

        pub fn in_rangef(&self, radius: f32) -> PoolVec<GameEntityId> {
            self.field.by_radiusf(&self.pos, radius)
        }

        /// Generally faster than `in_rangef`
        pub fn in_range(&self, radius: i32) -> PoolVec<GameEntityId> {
            self.field.by_radius(&self.pos, radius)
        }

        /// The returned set is not sorted in any way
        pub fn in_range_set(&self, radius: i32) -> PoolFxHashSet<GameEntityId> {
            self.field.by_radius_set(&self.pos, radius)
        }
    }

    impl Drop for CharacterPos {
        fn drop(&mut self) {
            self.field.remove(self.id);
        }
    }
}
