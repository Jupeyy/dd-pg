pub mod collision {
    use hiarc::Hiarc;
    use map::map::groups::layers::tiles::{TileBase, TuneTile};
    use serde::{Deserialize, Serialize};
    use shared_base::mapdef_06::DdraceTileNum;

    use math::math::{
        distance, length, mix, round_to_int,
        vector::{ivec2, vec2},
    };

    use crate::state::state::TICKS_PER_SECOND;

    #[derive(Debug, Hiarc, Copy, Clone, Serialize, Deserialize)]
    pub struct Tunings {
        pub ground_control_speed: f32,
        pub ground_control_accel: f32,
        pub ground_friction: f32,
        pub ground_jump_impulse: f32,
        pub air_jump_impulse: f32,
        pub air_control_speed: f32,
        pub air_control_accel: f32,
        pub air_friction: f32,
        pub hook_length: f32,
        pub hook_fire_speed: f32,
        pub hook_drag_accel: f32,
        pub hook_drag_speed: f32,
        pub gravity: f32,
        pub velramp_start: f32,
        pub velramp_range: f32,
        pub velramp_curvature: f32,
        pub gun_curvature: f32,
        pub gun_speed: f32,
        pub gun_lifetime: f32,
        pub shotgun_curvature: f32,
        pub shotgun_speed: f32,
        pub shotgun_speeddiff: f32,
        pub shotgun_lifetime: f32,
        pub grenade_curvature: f32,
        pub grenade_speed: f32,
        pub grenade_lifetime: f32,
        pub laser_reach: f32,
        pub laser_bounce_delay: f32,
        pub laser_bounce_num: f32,
        pub laser_bounce_cost: f32,
        pub laser_damage: f32,
        pub player_collision: f32,
        pub player_hooking: f32,
        pub jetpack_strength: f32,
        pub shotgun_strength: f32,
        pub explosion_strength: f32,
        pub hammer_strength: f32,
        pub hook_duration: f32,
        pub hammer_fire_delay: f32,
        pub gun_fire_delay: f32,
        pub shotgun_fire_delay: f32,
        pub grenade_fire_delay: f32,
        pub laser_fire_delay: f32,
        pub ninja_fire_delay: f32,
        pub hammer_hit_fire_delay: f32,
    }

    impl Default for Tunings {
        fn default() -> Self {
            Self {
                ground_control_speed: 10.0,
                ground_control_accel: 100.0 / TICKS_PER_SECOND as f32,
                ground_friction: 0.5,
                ground_jump_impulse: 13.2,
                air_jump_impulse: 12.0,
                air_control_speed: 250.0 / TICKS_PER_SECOND as f32,
                air_control_accel: 1.5,
                air_friction: 0.95,
                hook_length: 380.0,
                hook_fire_speed: 80.0,
                hook_drag_accel: 3.0,
                hook_drag_speed: 15.0,
                gravity: 0.5,
                velramp_start: 550.0,
                velramp_range: 2000.0,
                velramp_curvature: 1.4,
                gun_curvature: 1.25,
                gun_speed: 2200.0,
                gun_lifetime: 2.0,
                shotgun_curvature: 1.25,
                shotgun_speed: 2750.0,
                shotgun_speeddiff: 0.8,
                shotgun_lifetime: 0.20,
                grenade_curvature: 7.0,
                grenade_speed: 1000.0,
                grenade_lifetime: 2.0,
                laser_reach: 800.0,
                laser_bounce_delay: 150.0,
                laser_bounce_num: 1000.0,
                laser_bounce_cost: 0.0,
                laser_damage: 5.0,
                player_collision: 1.0,
                player_hooking: 1.0,
                jetpack_strength: 400.0,
                shotgun_strength: 10.0,
                explosion_strength: 6.0,
                hammer_strength: 1.0,
                hook_duration: 1.25,
                hammer_fire_delay: 125.0,
                gun_fire_delay: 125.0,
                shotgun_fire_delay: 500.0,
                grenade_fire_delay: 500.0,
                laser_fire_delay: 800.0,
                ninja_fire_delay: 800.0,
                hammer_hit_fire_delay: 320.0,
            }
        }
    }

    #[derive(Default)]
    pub struct Collision {
        tiles: Vec<TileBase>,
        tune_tiles: Vec<TuneTile>,
        width: u32,
        height: u32,

        tune_zones: Vec<Tunings>,
    }

    // TODO: use u8 or an enum for tile indices, instead of i32
    impl Collision {
        pub fn new(
            width: u32,
            height: u32,
            tiles: &[TileBase],
            tune_zones_and_tiles: Option<(Vec<Tunings>, &[TuneTile])>,
        ) -> Self {
            let mut tune_zones = vec![Tunings::default()];
            let tune_tiles: Vec<_> =
                if let Some((tune_zone_list, tune_tiles)) = tune_zones_and_tiles {
                    tune_zones = tune_zone_list.to_vec();
                    tune_tiles.to_vec()
                } else {
                    vec![TuneTile::default(); tiles.len()]
                };

            Self {
                width,
                height,
                tiles: tiles.to_vec(),
                tune_tiles,
                tune_zones,
            }
        }

        pub fn get_playfield_width(&self) -> u32 {
            self.width
        }

        pub fn get_playfield_height(&self) -> u32 {
            self.height
        }

        pub fn get_tile(&self, x: i32, y: i32) -> i32 {
            let nx = (x / 32).clamp(0, self.width as i32 - 1);
            let ny = (y / 32).clamp(0, self.height as i32 - 1);
            let pos = ny * self.width as i32 + nx;

            if self.tiles[pos as usize].index >= DdraceTileNum::Solid as u8
                && self.tiles[pos as usize].index <= DdraceTileNum::NoLaser as u8
            {
                return self.tiles[pos as usize].index as i32;
            }
            0
        }

        pub fn is_solid(&self, x: i32, y: i32) -> bool {
            let index = self.get_tile(x, y);
            index == DdraceTileNum::Solid as i32 || index == DdraceTileNum::NoHook as i32
        }

        pub fn is_death(&self, x: f32, y: f32) -> bool {
            let index = self.get_tile(round_to_int(x), round_to_int(y));
            index == DdraceTileNum::Death as i32
        }

        pub fn check_point(&self, x: i32, y: i32) -> bool {
            self.is_solid(x, y)
        }

        pub fn check_pointf(&self, x: f32, y: f32) -> bool {
            self.is_solid(round_to_int(x), round_to_int(y))
        }

        pub fn test_box(&self, pos: &ivec2, size_param: &ivec2) -> bool {
            let mut size = *size_param;
            size /= 2;
            self.check_point(pos.x - size.x, pos.y - size.y)
                || self.check_point(pos.x + size.x, pos.y - size.y)
                || self.check_point(pos.x - size.x, pos.y + size.y)
                || self.check_point(pos.x + size.x, pos.y + size.y)
        }

        pub fn move_point(
            &self,
            inout_pos: &mut vec2,
            inout_vel: &mut vec2,
            elasticity: f32,
            bounces: &mut i32,
        ) {
            *bounces = 0;

            let pos = *inout_pos;
            let vel = *inout_vel;
            let pos_vel = pos + vel;
            if self.check_pointf(pos_vel.x, pos_vel.y) {
                let mut affected = 0;
                if self.check_pointf(pos.x + vel.x, pos.y) {
                    inout_vel.x *= -elasticity;
                    *bounces += 1;
                    affected += 2;
                }

                if self.check_pointf(pos.x, pos.y + vel.y) {
                    inout_vel.y *= -elasticity;
                    *bounces += 1;
                    affected += 1;
                }

                if affected == 0 {
                    inout_vel.x *= -elasticity;
                    inout_vel.y *= -elasticity;
                }
            } else {
                *inout_pos = pos + vel;
            }
        }

        pub fn move_box(
            &self,
            in_out_pos: &mut vec2,
            in_out_vel: &mut vec2,
            size: &ivec2,
            elasticity: f32,
        ) {
            // do the move
            let mut pos = *in_out_pos;
            let mut vel = *in_out_vel;

            let vel_distance = length(&vel);
            let max = vel_distance as i32;

            if vel_distance > 0.00001 {
                let mut last_pos_x = round_to_int(pos.x);
                let mut last_pos_y = round_to_int(pos.y);
                let fraction = 1.0 / (max + 1) as f32;
                for _i in 0..=max {
                    // Early break as optimization to stop checking for collisions for
                    // large distances after the obstacles we have already hit reduced
                    // our speed to exactly 0.
                    if vel == vec2::new(0.0, 0.0) {
                        break;
                    }

                    let mut new_pos = pos + vel * fraction; // TODO: this row is not nice

                    // Fraction can be very small and thus the calculation has no effect, no
                    // reason to continue calculating.
                    if new_pos == pos {
                        break;
                    }

                    let mut new_pos_x = round_to_int(new_pos.x);
                    let mut new_pos_y = round_to_int(new_pos.y);

                    if self.test_box(&ivec2::new(new_pos_x, new_pos_y), size) {
                        let mut hits = 0;

                        if self.test_box(&ivec2::new(last_pos_x, new_pos_y), size) {
                            new_pos.y = pos.y;
                            new_pos_y = last_pos_y;
                            vel.y *= -elasticity;
                            hits += 1;
                        }

                        if self.test_box(&ivec2::new(new_pos_x, last_pos_y), size) {
                            new_pos.x = pos.x;
                            new_pos_x = last_pos_x;
                            vel.x *= -elasticity;
                            hits += 1;
                        }

                        // neither of the tests got a collision.
                        // this is a real _corner case_!
                        if hits == 0 {
                            new_pos.y = pos.y;
                            vel.y *= -elasticity;
                            new_pos.x = pos.x;
                            vel.x *= -elasticity;
                        }
                    }

                    last_pos_x = new_pos_x;
                    last_pos_y = new_pos_y;
                    pos = new_pos;
                }
            }

            *in_out_pos = pos;
            *in_out_vel = vel;
        }

        fn is_teleport(&self, _index: i32) -> i32 {
            /* TODO if (Index < 0 || !m_pTele) {
                return 0;
            }

            if m_pTele[Index].m_Type == TILE_TELEIN {
                return m_pTele[Index].m_Number;
            }*/

            0
        }

        fn is_teleport_hook(&self, _index: i32) -> i32 {
            /* TODO if Index < 0 || !m_pTele {
                return 0;
            }

            if m_pTele[Index].m_Type == TILE_TELEINHOOK {
                return m_pTele[Index].m_Number;
            }*/

            0
        }

        fn is_teleport_weapon(&self, _index: i32) -> i32 {
            /* TODO: if(Index < 0 || !m_pTele)
                return 0;

            if(m_pTele[Index].m_Type == TILE_TELEINWEAPON)
                return m_pTele[Index].m_Number;*/

            0
        }

        fn is_hook_blocker(&self, _x: i32, _y: i32, _pos0: &vec2, _pos1: &vec2) -> bool {
            /* TODO: let pos = self.GetPureMapIndex(x, y);
            if (m_pTiles[pos].m_Index == TILE_THROUGH_ALL
                || (m_pFront && m_pFront[pos].m_Index == TILE_THROUGH_ALL))
            {
                return true;
            }
            if (m_pTiles[pos].m_Index == TILE_THROUGH_DIR
                && ((m_pTiles[pos].m_Flags == ROTATION_0 && pos0.y < pos1.y)
                    || (m_pTiles[pos].m_Flags == ROTATION_90 && pos0.x > pos1.x)
                    || (m_pTiles[pos].m_Flags == ROTATION_180 && pos0.y > pos1.y)
                    || (m_pTiles[pos].m_Flags == ROTATION_270 && pos0.x < pos1.x)))
            {
                return true;
            }
            if (m_pFront
                && m_pFront[pos].m_Index == TILE_THROUGH_DIR
                && ((m_pFront[pos].m_Flags == ROTATION_0 && pos0.y < pos1.y)
                    || (m_pFront[pos].m_Flags == ROTATION_90 && pos0.x > pos1.x)
                    || (m_pFront[pos].m_Flags == ROTATION_180 && pos0.y > pos1.y)
                    || (m_pFront[pos].m_Flags == ROTATION_270 && pos0.x < pos1.x)))
            {
                return true;
            }*/
            false
        }

        fn is_through(
            &self,
            _x: i32,
            _y: i32,
            _xoff: i32,
            _yoff: i32,
            _pos0: &vec2,
            _pos1: &vec2,
        ) -> bool {
            /* TODO: let pos = self.GetPureMapIndex(x, y);
            if (m_pFront
                && (m_pFront[pos].m_Index == TILE_THROUGH_ALL
                    || m_pFront[pos].m_Index == TILE_THROUGH_CUT))
            {
                return true;
            }
            if (m_pFront
                && m_pFront[pos].m_Index == TILE_THROUGH_DIR
                && ((m_pFront[pos].m_Flags == ROTATION_0 && pos0.y > pos1.y)
                    || (m_pFront[pos].m_Flags == ROTATION_90 && pos0.x < pos1.x)
                    || (m_pFront[pos].m_Flags == ROTATION_180 && pos0.y < pos1.y)
                    || (m_pFront[pos].m_Flags == ROTATION_270 && pos0.x > pos1.x)))
            {
                return true;
            }
            let offpos = self.GetPureMapIndex(x + xoff, y + yoff);
            return m_pTiles[offpos].m_Index == TILE_THROUGH
                || (m_pFront && m_pFront[offpos].m_Index == TILE_THROUGH);*/
            false
        }

        fn get_collision_at(&self, x: f32, y: f32) -> i32 {
            self.get_tile(round_to_int(x), round_to_int(y))
        }

        fn get_pure_map_index(&self, x: f32, y: f32) -> i32 {
            let nx = (round_to_int(x) / 32).clamp(0, self.width as i32 - 1);
            let ny = (round_to_int(y) / 32).clamp(0, self.height as i32 - 1);
            ny * self.width as i32 + nx
        }

        fn tile_index(&self, x: f32, y: f32) -> usize {
            let nx = (round_to_int(x) / 32).clamp(0, self.width as i32 - 1);
            let ny = (round_to_int(y) / 32).clamp(0, self.height as i32 - 1);
            ny as usize * self.width as usize + nx as usize
        }

        fn through_offset(&self, pos0: &vec2, pos1: &vec2, offset_x: &mut i32, offset_y: &mut i32) {
            let x = pos0.x - pos1.x;
            let y = pos0.y - pos1.y;
            if x.abs() > y.abs() {
                if x < 0.0 {
                    *offset_x = -32;
                    *offset_y = 0;
                } else {
                    *offset_x = 32;
                    *offset_y = 0;
                }
            } else if y < 0.0 {
                *offset_x = 0;
                *offset_y = -32;
            } else {
                *offset_x = 0;
                *offset_y = 32;
            }
        }

        pub fn intersect_line_tele_hook(
            &self,
            pos0: &vec2,
            pos1: &vec2,
            out_collision: &mut vec2,
            out_before_collision: &mut vec2,
            tele_nr: &mut i32,
        ) -> i32 {
            let distance = distance(pos0, pos1);
            let end = (distance + 1.0) as i32;
            let mut last = *pos0;
            let mut dx = 0;
            let mut dy = 0; // Offset for checking the "through" tile
            self.through_offset(pos0, pos1, &mut dx, &mut dy);
            for i in 0..=end {
                let a = i as f32 / end as f32;
                let pos = mix(pos0, pos1, a);
                // Temporary position for checking collision
                let ix = round_to_int(pos.x);
                let iy = round_to_int(pos.y);

                let index = self.get_pure_map_index(pos.x, pos.y);
                if
                /* TODO: g_Config.m_SvOldTeleportHook*/
                false {
                    *tele_nr = self.is_teleport(index);
                } else {
                    *tele_nr = self.is_teleport_hook(index);
                }
                if *tele_nr > 0 {
                    *out_collision = pos;
                    *out_before_collision = last;
                    return DdraceTileNum::TeleInHook as i32; // TODO: dont like this hack, hit points and enum mixing
                }

                let mut hit = 0;
                if self.check_pointf(ix as f32, iy as f32) {
                    if !self.is_through(ix, iy, dx, dy, pos0, pos1) {
                        hit = self.get_collision_at(ix as f32, iy as f32);
                    }
                } else if self.is_hook_blocker(ix, iy, pos0, pos1) {
                    hit = DdraceTileNum::NoHook as i32; // TODO: dont like this hack, hit points and enum mixing
                }
                if hit > 0 {
                    *out_collision = pos;
                    *out_before_collision = last;
                    return hit;
                }

                last = pos;
            }
            *out_collision = *pos1;
            *out_before_collision = *pos1;
            0
        }

        pub fn intersect_line_tele_weapon(
            &self,
            pos0: &vec2,
            pos1: &vec2,
            out_collision: &mut vec2,
            out_before_collision: &mut vec2,
            tele_nr_out: &mut i32,
        ) -> i32 {
            let dist = distance(pos0, pos1);
            let end = (dist + 1.0) as i32;
            let mut last = *pos0;
            for i in 0..end {
                let a = i as f32 / end as f32;
                let pos = mix(pos0, pos1, a);
                // Temporary position for checking collision
                let ix = round_to_int(pos.x);
                let iy = round_to_int(pos.y);

                let index = self.get_pure_map_index(pos.x, pos.y);
                if false
                // TODO: (g_Config.m_SvOldTeleportWeapons)
                {
                    *tele_nr_out = self.is_teleport(index);
                } else {
                    *tele_nr_out = self.is_teleport_weapon(index);
                }
                if *tele_nr_out > 0 {
                    *out_collision = pos;
                    *out_before_collision = last;
                    return DdraceTileNum::TeleInWeapon as i32;
                }

                if self.check_pointf(ix as f32, iy as f32) {
                    *out_collision = pos;
                    *out_before_collision = last;
                    return self.get_collision_at(ix as f32, iy as f32);
                }

                last = pos;
            }
            *out_collision = *pos1;
            *out_before_collision = *pos1;
            0
        }

        pub fn intersect_line(
            &self,
            pos_0: &vec2,
            pos_1: &vec2,
            out_collision: &mut vec2,
            out_before_collision: &mut vec2,
        ) -> i32 {
            let d = distance(pos_0, pos_1);
            let end = (d + 1.0) as i32;
            let mut last_pos = *pos_0;
            for i in 0..=end {
                let a = i as f32 / end as f32;
                let pos = mix(pos_0, pos_1, a);
                // Temporary position for checking collision
                let ix = round_to_int(pos.x);
                let iy = round_to_int(pos.y);

                if self.check_pointf(ix as f32, iy as f32) {
                    *out_collision = pos;
                    *out_before_collision = last_pos;
                    return self.get_collision_at(ix as f32, iy as f32);
                }

                last_pos = pos;
            }
            *out_collision = *pos_1;
            *out_before_collision = *pos_1;
            0
        }

        pub fn get_tune_at(&self, pos: &vec2) -> &Tunings {
            let tune_tile = &self.tune_tiles[self.tile_index(pos.x, pos.y)];
            &self.tune_zones[tune_tile.number as usize]
        }
    }
}
