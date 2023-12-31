pub mod collision {
    use shared_base::mapdef::{CTile, TileNum};

    use math::math::{distance, length, mix, round_to_int, vector::vec2};

    #[derive(Default)]
    pub struct Collision {
        tiles: Vec<CTile>,
        width: u32,
        height: u32,
    }

    // TODO: use u8 or an enum for tile indices, instead of i32
    impl Collision {
        pub fn new(width: u32, height: u32, tiles: &[CTile]) -> Self {
            Self {
                width,
                height,
                tiles: tiles.to_vec(),
            }
        }

        pub fn get_playfield_width(&self) -> u32 {
            self.width
        }

        pub fn get_playfield_height(&self) -> u32 {
            self.height
        }

        pub fn get_tile(&self, x: i32, y: i32) -> i32 {
            if self.tiles.is_empty() {
                return 0;
            }

            let nx = (x / 32).clamp(0, self.width as i32 - 1);
            let ny = (y / 32).clamp(0, self.height as i32 - 1);
            let pos = ny * self.width as i32 + nx;

            if self.tiles[pos as usize].index >= TileNum::Solid as u8
                && self.tiles[pos as usize].index <= TileNum::NoLaser as u8
            {
                return self.tiles[pos as usize].index as i32;
            }
            return 0;
        }

        pub fn is_solid(&self, x: i32, y: i32) -> bool {
            let index = self.get_tile(x, y);
            return index == TileNum::Solid as i32 || index == TileNum::NoHook as i32;
        }

        pub fn check_point(&self, x: f32, y: f32) -> bool {
            return self.is_solid(round_to_int(x), round_to_int(y));
        }

        pub fn test_box(&self, pos: &vec2, size_param: &vec2) -> bool {
            let mut size = *size_param;
            size *= 0.5;
            if self.check_point(pos.x - size.x, pos.y - size.y) {
                return true;
            }
            if self.check_point(pos.x + size.x, pos.y - size.y) {
                return true;
            }
            if self.check_point(pos.x - size.x, pos.y + size.y) {
                return true;
            }
            if self.check_point(pos.x + size.x, pos.y + size.y) {
                return true;
            }
            return false;
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
            if self.check_point(pos_vel.x, pos_vel.y) {
                let mut affected = 0;
                if self.check_point(pos.x + vel.x, pos.y) {
                    inout_vel.x *= -elasticity;
                    *bounces += 1;
                    affected += 2;
                }

                if self.check_point(pos.x, pos.y + vel.y) {
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
            size: &vec2,
            elasticity: f32,
        ) {
            // do the move
            let mut pos = *in_out_pos;
            let mut vel = *in_out_vel;

            let vel_distance = length(&vel);
            let max = vel_distance as i32;

            if vel_distance > 0.00001 {
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

                    if self.test_box(&vec2::new(new_pos.x, new_pos.y), size) {
                        let mut hits = 0;

                        if self.test_box(&vec2::new(pos.x, new_pos.y), size) {
                            new_pos.y = pos.y;
                            vel.y *= -elasticity;
                            hits += 1;
                        }

                        if self.test_box(&vec2::new(new_pos.x, pos.y), size) {
                            new_pos.x = pos.x;
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
            return ny * self.width as i32 + nx;
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
            } else {
                if y < 0.0 {
                    *offset_x = 0;
                    *offset_y = -32;
                } else {
                    *offset_x = 0;
                    *offset_y = 32;
                }
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
                    return TileNum::TeleInHook as i32; // TODO: dont like this hack, hit points and enum mixing
                }

                let mut hit = 0;
                if self.check_point(ix as f32, iy as f32) {
                    if !self.is_through(ix, iy, dx, dy, pos0, pos1) {
                        hit = self.get_collision_at(ix as f32, iy as f32);
                    }
                } else if self.is_hook_blocker(ix, iy, pos0, pos1) {
                    hit = TileNum::NoHook as i32; // TODO: dont like this hack, hit points and enum mixing
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
            return 0;
        }

        fn IsTeleport(&self, Index: i32) -> i32 {
            /* TODO: if Index < 0 || !m_pTele
                return 0;

            if(m_pTele[Index].m_Type == TILE_TELEIN)
                return m_pTele[Index].m_Number;*/

            return 0;
        }

        fn IsTeleportWeapon(&self, Index: i32) -> i32 {
            /* TODO: if(Index < 0 || !m_pTele)
                return 0;

            if(m_pTele[Index].m_Type == TILE_TELEINWEAPON)
                return m_pTele[Index].m_Number;*/

            return 0;
        }

        pub fn intersect_line_tele_weapon(
            &self,
            Pos0: &vec2,
            Pos1: &vec2,
            pOutCollision: &mut vec2,
            pOutBeforeCollision: &mut vec2,
            pTeleNr: &mut i32,
        ) -> i32 {
            let Distance = distance(Pos0, Pos1);
            let End = (Distance + 1.0) as i32;
            let mut Last = *Pos0;
            for i in 0..End {
                let a = i as f32 / End as f32;
                let Pos = mix(Pos0, Pos1, a);
                // Temporary position for checking collision
                let ix = round_to_int(Pos.x);
                let iy = round_to_int(Pos.y);

                let Index = self.get_pure_map_index(Pos.x, Pos.y);
                if false
                // TODO: (g_Config.m_SvOldTeleportWeapons)
                {
                    *pTeleNr = self.IsTeleport(Index);
                } else {
                    *pTeleNr = self.IsTeleportWeapon(Index);
                }
                if *pTeleNr > 0 {
                    *pOutCollision = Pos;
                    *pOutBeforeCollision = Last;
                    return TileNum::TeleInWeapon as i32;
                }

                if self.check_point(ix as f32, iy as f32) {
                    *pOutCollision = Pos;
                    *pOutBeforeCollision = Last;
                    return self.get_collision_at(ix as f32, iy as f32);
                }

                Last = Pos;
            }
            *pOutCollision = *Pos1;
            *pOutBeforeCollision = *Pos1;
            return 0;
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

                if self.check_point(ix as f32, iy as f32) {
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
    }
}
