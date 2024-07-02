use std::time::Duration;

use graphics_types::rendering::ColorRGBA;
use math::math::{
    mix,
    vector::{vec2, vec4},
    Rng, PI,
};

use super::{
    particle::Particle,
    particle_manager::{ParticleGroup, ParticleManager},
};

pub struct Effects<'a> {
    particle_manager: &'a mut ParticleManager,
    rate_5_time: Duration,
    rate_50_time: Duration,
    rate_100_time: Duration,
}

impl<'a> Effects<'a> {
    pub fn new(particle_manager: &'a mut ParticleManager, cur_time: Duration) -> Self {
        Self {
            particle_manager,

            rate_5_time: Duration::from_nanos(
                ((cur_time.as_nanos() / Duration::from_millis(1000 / 5).as_nanos())
                    * Duration::from_millis(1000 / 5).as_nanos()) as u64,
            ),
            rate_50_time: Duration::from_nanos(
                ((cur_time.as_nanos() / Duration::from_millis(1000 / 50).as_nanos())
                    * Duration::from_millis(1000 / 50).as_nanos()) as u64,
            ),
            rate_100_time: Duration::from_nanos(
                ((cur_time.as_nanos() / Duration::from_millis(1000 / 100).as_nanos())
                    * Duration::from_millis(1000 / 100).as_nanos()) as u64,
            ),
        }
    }

    fn random_dir(rng: &mut Rng) -> vec2 {
        let angle = 2.0 * PI * rng.random_float();
        vec2::new(angle.cos(), angle.sin())
    }

    pub fn air_jump(&mut self, pos: &vec2) {
        let mut p = Particle::default();
        p.texture = "airjump";
        p.pos = *pos + vec2::new(-6.0, 16.0) / 32.0;
        p.vel = vec2::new(0.0, -200.0 / 32.0);
        p.life_span = 0.5;
        p.start_size = 1.5;
        p.end_size = 0.0;
        p.rot = self.particle_manager.rng.random_float() * PI * 2.0;
        p.rot_speed = PI * 2.0;
        p.gravity = 500.0 / 32.0;
        p.friction = 0.7;
        p.flow_affected = 0.0;
        self.particle_manager
            .add(ParticleGroup::General, p.clone(), 0.0);

        p.pos = *pos + vec2::new(6.0, 16.0) / 32.0;
        self.particle_manager.add(ParticleGroup::General, p, 0.0);
    }
    /*
    void CEffects::DamageIndicator(vec2 pos, vec2 Dir)
    {
        m_pClient->m_DamageInd.Create(pos, Dir);
    }

    void CEffects::ResetDamageIndicator()
    {
        m_pClient->m_DamageInd.Reset();
    }*/

    pub fn powerup_shine(&mut self, pos: &vec2, size: &vec2) {
        if self.rate_50_time < self.particle_manager.last_50_time {
            return;
        }

        let mut p = Particle::default();
        p.texture = "slice";
        p.pos = *pos
            + vec2::new(
                (self.particle_manager.rng.random_float() - 0.5) * size.x,
                (self.particle_manager.rng.random_float() - 0.5) * size.y,
            );
        p.vel = vec2::new(0.0, 0.0);
        p.life_span = 0.5;
        p.start_size = 0.5;
        p.end_size = 0.0;
        p.rot = self.particle_manager.rng.random_float() * PI * 2.0;
        p.rot_speed = PI * 2.0;
        p.gravity = 500.0 / 32.0;
        p.friction = 0.9;
        p.flow_affected = 0.0;
        self.particle_manager.add(ParticleGroup::General, p, 0.0);
    }

    pub fn freezing_flakes(&mut self, pos: &vec2, size: &vec2) {
        if self.rate_5_time < self.particle_manager.last_5_time {
            return;
        }

        let rng = &mut self.particle_manager.rng;

        let mut p = Particle::default();
        p.texture = "snowflake";
        p.pos = *pos
            + vec2::new(
                (rng.random_float() - 0.5) * size.x,
                (rng.random_float() - 0.5) * size.y,
            );
        p.vel = vec2::default();
        p.life_span = 1.5;
        p.start_size = (rng.random_float() + 0.5) * 0.5;
        p.end_size = p.start_size * 0.5;
        p.use_alpha_fading = true;
        p.start_alpha = 1.0;
        p.end_alpha = 0.0;
        p.rot = rng.random_float() * PI * 2.0;
        p.rot_speed = PI;
        p.gravity = rng.random_float() * 250.0 / 32.0;
        p.friction = 0.9;
        p.flow_affected = 0.0;
        p.collides = false;
        self.particle_manager.add(ParticleGroup::Extra, p, 0.0);
    }

    pub fn smoke_trail(&mut self, pos: &vec2, vel: &vec2, alpha: f32, time_passed: f32) {
        if self.rate_50_time < self.particle_manager.last_50_time {
            return;
        }

        let mut p = Particle::default();
        p.texture = "smoke";
        p.pos = *pos;
        p.vel = *vel + (Self::random_dir(&mut self.particle_manager.rng) * 50.0 / 32.0);
        p.life_span = 0.5 + self.particle_manager.rng.random_float() * 0.5;
        p.start_size = 3.0 / 8.0 + self.particle_manager.rng.random_float() * 0.25;
        p.end_size = 0.0;
        p.friction = 0.7;
        p.gravity = (self.particle_manager.rng.random_float() * -500.0) / 32.0;
        p.color.a *= alpha;
        self.particle_manager
            .add(ParticleGroup::ProjectileTrail, p, time_passed);
    }

    pub fn skid_trail(&mut self, pos: &vec2, vel: &vec2) {
        if self.rate_100_time < self.particle_manager.last_100_time {
            return;
        }

        let rng = &mut self.particle_manager.rng;
        let mut p = Particle::default();
        p.texture = "smoke";
        p.pos = *pos;
        p.vel = *vel + Self::random_dir(rng) * 50.0 / 32.0;
        p.life_span = 0.5 + rng.random_float() * 0.5;
        p.start_size = 0.75 + rng.random_float() * 12.0 / 32.0;
        p.end_size = 0.0;
        p.friction = 0.7;
        p.gravity = rng.random_float() * -500.0 / 32.0;
        p.color = ColorRGBA::new(0.75, 0.75, 0.75, 1.0);
        self.particle_manager.add(ParticleGroup::General, p, 0.0);
    }

    pub fn bullet_trail(&mut self, pos: &vec2, alpha: f32) {
        if self.rate_100_time < self.particle_manager.last_100_time {
            return;
        }

        let mut p = Particle::default();
        p.texture = "ball";
        p.pos = *pos;
        p.life_span = 0.25 + self.particle_manager.rng.random_float() * 0.25;
        p.start_size = 8.0 / 32.0;
        p.end_size = 0.0;
        p.friction = 0.7;
        p.color.a *= alpha;
        self.particle_manager
            .add(ParticleGroup::ProjectileTrail, p, 0.0);
    }

    pub fn player_spawn(&mut self, pos: &vec2) {
        for _ in 0..32 {
            let rng = &mut self.particle_manager.rng;
            let mut p = Particle::default();
            p.texture = "shell";
            p.pos = *pos;
            p.vel = Self::random_dir(rng) * (rng.random_float().powf(3.0) * 600.0 / 32.0);
            p.life_span = 0.3 + rng.random_float() * 0.3;
            p.start_size = 2.0 + rng.random_float();
            p.end_size = 0.0;
            p.rot = rng.random_float() * PI * 2.0;
            p.rot_speed = rng.random_float();
            p.gravity = rng.random_float() * -400.0 / 32.0;
            p.friction = 0.7;
            p.color = ColorRGBA::new(
                0xb5 as f32 / 255.0,
                0x50 as f32 / 255.0,
                0xcb as f32 / 255.0,
                1.0,
            );
            self.particle_manager.add(ParticleGroup::General, p, 0.0);
        }
    }

    pub fn player_death(&mut self, pos: &vec2, bloor_color: ColorRGBA) {
        for _ in 0..64 {
            let rng = &mut self.particle_manager.rng;
            let mut p = Particle::default();
            p.texture = match rng.random_int_in(0..=2) {
                0 => "splat0",
                1 => "splat1",
                _ => "splat2",
            };
            p.pos = *pos;
            p.vel = Self::random_dir(rng) * ((rng.random_float() + 0.1) * 900.0 / 32.0);
            p.life_span = 0.3 + rng.random_float() * 0.3;
            p.start_size = 0.75 + rng.random_float() * 0.5;
            p.end_size = 0.0;
            p.rot = rng.random_float() * PI * 2.0;
            p.rot_speed = (rng.random_float() - 0.5) * PI;
            p.gravity = 25.0;
            p.friction = 0.8;
            let c = vec4::new(bloor_color.r, bloor_color.g, bloor_color.b, bloor_color.a)
                * (0.75 + rng.random_float() * 0.25);
            p.color = ColorRGBA::new(c.r(), c.g(), c.b(), 0.75);
            self.particle_manager.add(ParticleGroup::General, p, 0.0);
        }
    }

    pub fn explosion(&mut self, pos: &vec2) {
        // add to flow
        /*for y in -8..=8 {
            for x in -8..=8 {
                if x == 0 && y == 0 {
                    continue;
                }

                let a = 1.0 - (length(&vec2::new(x as f32, y as f32)) / length(&vec2::new(8.0, 8.0)));
                m_pClient->m_Flow.Add(pos + vec2(x, y) * 16, normalize(vec2(x, y)) * 5000.0 * a, 10.0);
            }
        }*/

        let rng = &mut self.particle_manager.rng;
        // add the explosion
        let mut p = Particle::default();
        p.texture = "explosion0";
        p.pos = *pos;
        p.life_span = 0.4;
        p.start_size = 150.0 / 32.0;
        p.end_size = 0.0;
        p.rot = rng.random_float() * PI * 2.0;
        self.particle_manager.add(ParticleGroup::Explosions, p, 0.0);

        // add the smoke
        for _ in 0..24 {
            let rng = &mut self.particle_manager.rng;
            let mut p = Particle::default();
            p.texture = "smoke";
            p.pos = *pos;
            p.vel = Self::random_dir(rng) * ((1.0 + rng.random_float() * 0.2) * 1000.0 / 32.0);
            p.life_span = 0.5 + rng.random_float() * 0.4;
            p.start_size = 1.0 + rng.random_float() * 0.25;
            p.end_size = 0.0;
            p.gravity = rng.random_float() * -25.0;
            p.friction = 0.4;
            let color = mix(
                &vec4::new(0.75, 0.75, 0.75, 1.0),
                &vec4::new(0.5, 0.5, 0.5, 1.0),
                rng.random_float(),
            );
            p.color = ColorRGBA::new(color.x, color.y, color.z, color.w);
            self.particle_manager.add(ParticleGroup::General, p, 0.0);
        }
    }

    pub fn hammer_hit(&mut self, pos: &vec2) {
        let rng = &mut self.particle_manager.rng;
        // add the explosion
        let mut p = Particle::default();
        p.texture = "hit0";
        p.pos = *pos;
        p.life_span = 0.3;
        p.start_size = 120.0 / 32.0;
        p.end_size = 0.0;
        p.rot = rng.random_float() * PI * 2.0;
        self.particle_manager.add(ParticleGroup::Explosions, p, 0.0);
    }

    pub fn damage_ind(&mut self, pos: &vec2, vel: &vec2) {
        let rng = &mut self.particle_manager.rng;
        // add the explosion
        let mut p = Particle::default();
        p.texture = "star0";
        p.pos = *pos;
        p.vel = *vel * 1.0;
        p.life_span = 0.75;
        p.max_lifetime_vel = 0.15;
        p.start_size = 1.0;
        p.end_size = 1.0;
        p.rot = (rng.random_float() - 1.0) * PI * 2.0;
        p.rot_speed = 2.0;
        p.gravity = 0.0;
        p.friction = 1.0;
        p.collides = false;
        self.particle_manager.add(ParticleGroup::Explosions, p, 0.0);
    }
}
