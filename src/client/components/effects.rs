use math::math::{normalize, random_float, vector::vec2, PI};

use super::{
    particle::Particle,
    particle_manager::{ParticleGroup, ParticleManager},
};

pub struct Effects<'a> {
    particle_manager: &'a mut ParticleManager,
}

impl<'a> Effects<'a> {
    pub fn new(particle_manager: &'a mut ParticleManager) -> Self {
        Self { particle_manager }
    }

    fn random_dir() -> vec2 {
        normalize(&vec2::new(random_float() - 0.5, random_float() - 0.5))
    }

    pub fn _air_jump(&mut self, pos: &vec2) {
        let mut p = Particle::default();
        p.texture = "airjump";
        p.pos = *pos + vec2::new(-6.0, 16.0);
        p.vel = vec2::new(0.0, -200.0);
        p.life_span = 0.5;
        p.start_size = 48.0;
        p.end_size = 0.0;
        p.rot = random_float() * PI * 2.0;
        p.rot_speed = PI * 2.0;
        p.gravity = 500.0;
        p.friction = 0.7;
        p.flow_affected = 0.0;
        self.particle_manager
            .add(ParticleGroup::General, p.clone(), 0.0);

        p.pos = *pos + vec2::new(6.0, 16.0);
        self.particle_manager.add(ParticleGroup::General, p, 0.0);

        /* TODO: if g_Config.SndGame {
            m_pClient->m_Sounds.PlayAt(CSounds::CHN_WORLD, SOUND_PLAYER_AIRJUMP, 1.0, Pos);
        }*/
    }
    /*
    void CEffects::DamageIndicator(vec2 Pos, vec2 Dir)
    {
        m_pClient->m_DamageInd.Create(Pos, Dir);
    }

    void CEffects::ResetDamageIndicator()
    {
        m_pClient->m_DamageInd.Reset();
    }

    void CEffects::PowerupShine(vec2 Pos, vec2 Size)
    {
        if(!m_Add50hz)
            return;

        Particle p;
        p.SetDefault();
        p.Spr = SPRITE_PART_SLICE;
        p.Pos = Pos + vec2((random_float() - 0.5) * Size.x, (random_float() - 0.5) * Size.y);
        p.Vel = vec2(0, 0);
        p.LifeSpan = 0.5;
        p.StartSize = 16.0;
        p.EndSize = 0;
        p.Rot = random_float() * pi * 2;
        p.Rotspeed = pi * 2;
        p.Gravity = 500;
        p.Friction = 0.9;
        p.FlowAffected = 0.0;
        m_pClient->m_Particles.Add(CParticles::GROUP_GENERAL, &p);
    }

    void CEffects::FreezingFlakes(vec2 Pos, vec2 Size)
    {
        if(!m_Add5hz)
            return;

        Particle p;
        p.SetDefault();
        p.Spr = SPRITE_PART_SNOWFLAKE;
        p.Pos = Pos + vec2((random_float() - 0.5) * Size.x, (random_float() - 0.5) * Size.y);
        p.Vel = vec2(0, 0);
        p.LifeSpan = 1.5;
        p.StartSize = (random_float() + 0.5) * 16.0;
        p.EndSize = p.StartSize * 0.5;
        p.UseAlphaFading = true;
        p.StartAlpha = 1.0;
        p.EndAlpha = 0.0;
        p.Rot = random_float() * pi * 2;
        p.Rotspeed = pi;
        p.Gravity = random_float() * 250.0;
        p.Friction = 0.9;
        p.FlowAffected = 0.0;
        p.Collides = false;
        m_pClient->m_Particles.Add(CParticles::GROUP_EXTRA, &p);
    }*/

    pub fn smoke_trail(&mut self, pos: &vec2, vel: &vec2, alpha: f32, time_passed: f32) {
        // TODO: if(!m_Add50hz && TimePassed < 0.001)
        // TODO:             return;

        let mut p = Particle::default();
        p.texture = "smoke";
        p.pos = *pos;
        p.vel = *vel + Self::random_dir() * 50.0;
        p.life_span = 0.5 + random_float() * 0.5;
        p.start_size = 12.0 + random_float() * 8.0;
        p.end_size = 0.0;
        p.friction = 0.7;
        p.gravity = random_float() * -500.0;
        p.color.a *= alpha;
        self.particle_manager
            .add(ParticleGroup::ProjectileTrail, p, time_passed);
    }

    /*
    void CEffects::SkidTrail(vec2 Pos, vec2 Vel)
    {
        if(!m_Add100hz)
            return;

        Particle p;
        p.SetDefault();
        p.Spr = SPRITE_PART_SMOKE;
        p.Pos = Pos;
        p.Vel = Vel + RandomDir() * 50.0;
        p.LifeSpan = 0.5 + random_float() * 0.5;
        p.StartSize = 24.0 + random_float() * 12;
        p.EndSize = 0;
        p.Friction = 0.7;
        p.Gravity = random_float() * -500.0;
        p.Color = ColorRGBA(0.75, 0.75, 0.75, 1.0);
        m_pClient->m_Particles.Add(CParticles::GROUP_GENERAL, &p);
    }*/

    pub fn bullet_trail(&mut self, pos: &vec2, alpha: f32, time_passed: f32) {
        /* TODO: if(!m_Add100hz && TimePassed < 0.001)
        return;*/

        let mut p = Particle::default();
        p.texture = "ball";
        p.pos = *pos;
        p.life_span = 0.25 + random_float() * 0.25;
        p.start_size = 8.0;
        p.end_size = 0.0;
        p.friction = 0.7;
        p.color.a *= alpha;
        self.particle_manager
            .add(ParticleGroup::ProjectileTrail, p, time_passed);
    }

    /*void CEffects::PlayerSpawn(vec2 Pos)
    {
        for(int i = 0; i < 32; i++)
        {
            Particle p;
            p.SetDefault();
            p.Spr = SPRITE_PART_SHELL;
            p.Pos = Pos;
            p.Vel = RandomDir() * (powf(random_float(), 3) * 600.0);
            p.LifeSpan = 0.3 + random_float() * 0.3;
            p.StartSize = 64.0 + random_float() * 32;
            p.EndSize = 0;
            p.Rot = random_float() * pi * 2;
            p.Rotspeed = random_float();
            p.Gravity = random_float() * -400.0;
            p.Friction = 0.7;
            p.Color = ColorRGBA(0xb5 / 255.0, 0x50 / 255.0, 0xcb / 255.0, 1.0);
            m_pClient->m_Particles.Add(CParticles::GROUP_GENERAL, &p);
        }
        if(g_Config.SndGame)
            m_pClient->m_Sounds.PlayAt(CSounds::CHN_WORLD, SOUND_PLAYER_SPAWN, 1.0, Pos);
    }

    void CEffects::PlayerDeath(vec2 Pos, int ClientID)
    {
        ColorRGBA BloodColor(1.0, 1.0, 1.0);

        if(ClientID >= 0)
        {
            // Use m_RenderInfo.CustomColoredSkin instead of m_UseCustomColor
            // m_UseCustomColor says if the player's skin has a custom color (value sent from the client side)

            // m_RenderInfo.CustomColoredSkin Defines if in the context of the game the color is being customized,
            // Using this value if the game is teams (red and blue), this value will be true even if the skin is with the normal color.
            // And will use the team body color to create player death effect instead of tee color
            if(m_pClient->m_aClients[ClientID].RenderInfo.CustomColoredSkin)
                BloodColor = m_pClient->m_aClients[ClientID].RenderInfo.ColorBody;
            else
            {
                BloodColor = m_pClient->m_aClients[ClientID].RenderInfo.BloodColor;
            }
        }

        for(int i = 0; i < 64; i++)
        {
            Particle p;
            p.SetDefault();
            p.Spr = SPRITE_PART_SPLAT01 + (rand() % 3);
            p.Pos = Pos;
            p.Vel = RandomDir() * ((random_float() + 0.1) * 900.0);
            p.LifeSpan = 0.3 + random_float() * 0.3;
            p.StartSize = 24.0 + random_float() * 16;
            p.EndSize = 0;
            p.Rot = random_float() * pi * 2;
            p.Rotspeed = (random_float() - 0.5) * pi;
            p.Gravity = 800.0;
            p.Friction = 0.8;
            ColorRGBA c = BloodColor.v4() * (0.75 + random_float() * 0.25);
            p.Color = ColorRGBA(c.r, c.g, c.b, 0.75);
            m_pClient->m_Particles.Add(CParticles::GROUP_GENERAL, &p);
        }
    }

    void CEffects::Explosion(vec2 Pos)
    {
        // add to flow
        for(int y = -8; y <= 8; y++)
            for(int x = -8; x <= 8; x++)
            {
                if(x == 0 && y == 0)
                    continue;

                float a = 1 - (length(vec2(x, y)) / length(vec2(8, 8)));
                m_pClient->m_Flow.Add(Pos + vec2(x, y) * 16, normalize(vec2(x, y)) * 5000.0 * a, 10.0);
            }

        // add the explosion
        Particle p;
        p.SetDefault();
        p.Spr = SPRITE_PART_EXPL01;
        p.Pos = Pos;
        p.LifeSpan = 0.4;
        p.StartSize = 150.0;
        p.EndSize = 0;
        p.Rot = random_float() * pi * 2;
        m_pClient->m_Particles.Add(CParticles::GROUP_EXPLOSIONS, &p);

        // add the smoke
        for(int i = 0; i < 24; i++)
        {
            p.SetDefault();
            p.Spr = SPRITE_PART_SMOKE;
            p.Pos = Pos;
            p.Vel = RandomDir() * ((1.0 + random_float() * 0.2) * 1000.0);
            p.LifeSpan = 0.5 + random_float() * 0.4;
            p.StartSize = 32.0 + random_float() * 8;
            p.EndSize = 0;
            p.Gravity = random_float() * -800.0;
            p.Friction = 0.4;
            p.Color = mix(vec4(0.75, 0.75, 0.75, 1.0), vec4(0.5, 0.5, 0.5, 1.0), random_float());
            m_pClient->m_Particles.Add(CParticles::GROUP_GENERAL, &p);
        }
    }

    void CEffects::HammerHit(vec2 Pos)
    {
        // add the explosion
        Particle p;
        p.SetDefault();
        p.Spr = SPRITE_PART_HIT01;
        p.Pos = Pos;
        p.LifeSpan = 0.3;
        p.StartSize = 120.0;
        p.EndSize = 0;
        p.Rot = random_float() * pi * 2;
        m_pClient->m_Particles.Add(CParticles::GROUP_EXPLOSIONS, &p);
        if(g_Config.SndGame)
            m_pClient->m_Sounds.PlayAt(CSounds::CHN_WORLD, SOUND_HAMMER_HIT, 1.0, Pos);
    } */
}
