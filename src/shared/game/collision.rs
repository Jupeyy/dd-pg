use crate::mapdef::{CTile, TileNum};

use math::math::{length, round_to_int, vector::vec2};

#[derive(Default)]
pub struct Collision {
    tiles: Vec<CTile>,
    width: u32,
    height: u32,
}

impl Collision {
    pub fn new(width: u32, height: u32, tiles: &[CTile]) -> Self {
        Self {
            width,
            height,
            tiles: tiles.to_vec(),
        }
    }

    pub fn get_tile(&self, x: i32, y: i32) -> i32 {
        if self.tiles.is_empty() {
            return 0;
        }

        let nx = (x / 32).clamp(0, self.width as i32 - 1);
        let ny = (y / 32).clamp(0, self.height as i32 - 1);
        let pos = ny * self.width as i32 + nx;

        if self.tiles[pos as usize].index >= TileNum::TILE_SOLID as u8
            && self.tiles[pos as usize].index <= TileNum::TILE_NOLASER as u8
        {
            return self.tiles[pos as usize].index as i32;
        }
        return 0;
    }

    pub fn is_solid(&self, x: i32, y: i32) -> bool {
        let index = self.get_tile(x, y);
        return index == TileNum::TILE_SOLID as i32 || index == TileNum::TILE_NOHOOK as i32;
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
}
