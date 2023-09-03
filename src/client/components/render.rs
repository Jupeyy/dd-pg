use std::{collections::HashMap, sync::Arc, time::Duration};

use base::system::{System, SystemTimeInterface};
use base_fs::{filesys::FileSystem, io_batcher::TokIOBatcher};

use graphics_backend::types::Graphics;
use graphics_base::{
    quad_container::{GraphicsQuadContainerHandleInterface, QuadContainerIndex, SQuad},
    streaming::quad_scope_begin,
};
use graphics_types::rendering::{ColorRGBA, State};
use math::math::{
    angle, distance, length, normalize_pre_length,
    vector::{ubvec4, vec2},
    PI_F64,
};
use num_traits::FromPrimitive;
use shared_base::{game_types::intra_tick_time_to_ratio, types::GameTickType};
use shared_game::{
    entities::{
        flag::flag::FlagRenderInfo,
        laser::laser::LaserRenderInfo,
        pickup::pickup::{PickupRenderInfo, PickupType},
        projectile::projectile::ProjectileRenderInfo,
    },
    state::state::GameStateInterface,
    weapons::definitions::weapon_def::WeaponType,
};

use crate::{
    client::{client::ClientData, render_pipe::Camera},
    client_map::ClientMapFile,
    containers::{ctf::CTFContainer, pickups::PickupContainer, weapons::WeaponContainer},
};

use super::{
    particle_manager::ParticleManager,
    players::{get_weapon_as_quad, Players},
};

pub struct Render {
    _cur_time: Duration,

    items_quad_container: QuadContainerIndex,

    // offsets
    _ctf_flag_offset: usize, // TODO
    projectile_sprite_offset: usize,
    pickup_sprite_off: usize,
    particle_splat_off: usize,

    // helpers
    projs_helper: Vec<ProjectileRenderInfo>,
    lasers_helper: Vec<LaserRenderInfo>,
    pickups_helper: Vec<PickupRenderInfo>,
    ctf_flags_helper: Vec<FlagRenderInfo>,
}

pub fn get_sprite_scale_impl(w: u32, h: u32) -> (f32, f32) {
    let f = ((w * w + h * h) as f32).sqrt();
    (w as f32 / f, h as f32 / f)
}

pub fn pickup_scale() -> (f32, f32) {
    let grid_size = (2, 2);
    get_sprite_scale_impl(grid_size.0, grid_size.1)
}

pub struct RenderPipe<'a> {
    pub effects: &'a mut ParticleManager,
    pub sys: &'a System,
    pub graphics: &'a mut Graphics,
    pub client_data: &'a ClientData,
    pub cur_tick: GameTickType,

    pub map: &'a ClientMapFile,

    pub ctf_container: &'a mut CTFContainer,
    pub pickup_container: &'a mut PickupContainer,
    pub weapon_container: &'a mut WeaponContainer,

    pub camera: &'a Camera,

    pub fs: &'a Arc<FileSystem>,
    pub io_batcher: &'a TokIOBatcher,
}

impl Render {
    pub fn new(sys: &System, graphics: &mut Graphics) -> Self {
        let items_quad_container = graphics.quad_container_handle.create_quad_container(false);

        let quad = SQuad::new()
            .from_rect(-21.0, -42.0, 42.0, 84.0)
            .with_color(&ubvec4::new(255, 255, 255, 255))
            .with_uv_from_points(&vec2::new(0.0, 0.0), &vec2::new(1.0, 1.0));

        let ctf_flag_offset = graphics
            .quad_container_handle
            .quad_container_add_quads(&items_quad_container, &[quad]);

        let sprite_scale = pickup_scale();
        let quad = SQuad::new()
            .from_width_and_height_centered(64.0 * sprite_scale.0, 64.0 * sprite_scale.1)
            .with_color(&ubvec4::new(255, 255, 255, 255))
            .with_uv_from_points(&vec2::new(0.0, 0.0), &vec2::new(1.0, 1.0));

        let pickup_sprite_off = graphics
            .quad_container_handle
            .quad_container_add_quads(&items_quad_container, &[quad]);

        let mut weapon_quad_offsets: HashMap<WeaponType, usize> = Default::default();

        weapon_quad_offsets.reserve(WeaponType::NumWeapons as usize);
        (0..WeaponType::NumWeapons as usize)
            .enumerate()
            .for_each(|(index, wi)| {
                let quad = get_weapon_as_quad(&FromPrimitive::from_usize(wi).unwrap())
                    .with_color(&ubvec4::new(255, 255, 255, 255))
                    .with_uv_from_points(&vec2::new(0.0, 0.0), &vec2::new(1.0, 1.0));
                let offset_normal = graphics
                    .quad_container_handle
                    .quad_container_add_quads(&items_quad_container, &[quad]);
                weapon_quad_offsets.insert(WeaponType::from_usize(index).unwrap(), offset_normal);
            });

        /*RenderTools()->GetSpriteScale(SPRITE_PICKUP_NINJA, ScaleX, ScaleY);
        Graphics()->QuadsSetSubset(0, 0, 1, 1);
        m_PickupNinjaOffset = RenderTools()->QuadContainerAddSprite(items_quad_container, 128.f * ScaleX, 128.f * ScaleY);*/

        let quad = SQuad::new()
            .from_width_and_height_centered(32.0, 32.0)
            .with_color(&ubvec4::new(255, 255, 255, 255))
            .with_uv_from_points(&vec2::new(0.0, 0.0), &vec2::new(1.0, 1.0));

        let projectile_sprite_off = graphics
            .quad_container_handle
            .quad_container_add_quads(&items_quad_container, &[quad]);

        let quad = SQuad::new()
            .from_width_and_height_centered(24.0 * sprite_scale.0, 24.0 * sprite_scale.1)
            .with_color(&ubvec4::new(255, 255, 255, 255))
            .with_uv_from_points(&vec2::new(0.0, 0.0), &vec2::new(1.0, 1.0));

        let particle_splat_off = graphics
            .quad_container_handle
            .quad_container_add_quads(&items_quad_container, &[quad]);

        graphics
            .quad_container_handle
            .quad_container_upload(&items_quad_container);

        Self {
            _cur_time: sys.time_get_nanoseconds(),
            items_quad_container,

            _ctf_flag_offset: ctf_flag_offset,
            projectile_sprite_offset: projectile_sprite_off,
            pickup_sprite_off,
            particle_splat_off,

            projs_helper: Default::default(),
            lasers_helper: Default::default(),
            pickups_helper: Default::default(),
            ctf_flags_helper: Default::default(),
        }
    }

    pub fn render(&mut self, pipe: &mut RenderPipe) {
        let mut base_state = State::default();
        let center = pipe.camera.pos;
        Players::map_canvas_for_players(
            &pipe.graphics,
            &mut base_state,
            center.x,
            center.y,
            pipe.camera.zoom,
        );

        let ticks_per_second = pipe.map.game.game_tick_speed();
        let intra_tick_ratio =
            intra_tick_time_to_ratio(pipe.client_data.intra_tick_time, ticks_per_second);

        let mut projs = Default::default();
        std::mem::swap(&mut projs, &mut self.projs_helper);
        let mut flags = Default::default();
        std::mem::swap(&mut flags, &mut self.ctf_flags_helper);
        let mut lasers = Default::default();
        std::mem::swap(&mut lasers, &mut self.lasers_helper);
        let mut pickups = Default::default();
        std::mem::swap(&mut pickups, &mut self.pickups_helper);

        pipe.map.game.all_projectiles(intra_tick_ratio, &mut projs);
        projs.drain(..).for_each(|proj| {
            self.render_projectile(pipe, &proj, -1, &base_state);
        });
        pipe.map.game.all_ctf_flags(intra_tick_ratio, &mut flags);
        flags.drain(..).for_each(|flag| {
            self.render_flag(pipe, &flag, &base_state);
        });
        pipe.map.game.all_lasers(intra_tick_ratio, &mut lasers);
        lasers.drain(..).for_each(|laser| {
            self.render_laser(pipe, &laser, &base_state);
        });
        pipe.map.game.all_pickups(intra_tick_ratio, &mut pickups);
        pickups.drain(..).for_each(|pickup| {
            self.render_pickup(pipe, &pickup, &base_state);
        });

        std::mem::swap(&mut projs, &mut self.projs_helper);
        std::mem::swap(&mut flags, &mut self.ctf_flags_helper);
        std::mem::swap(&mut lasers, &mut self.lasers_helper);
        std::mem::swap(&mut pickups, &mut self.pickups_helper);
    }

    pub fn render_projectile(
        &mut self,
        pipe: &mut RenderPipe,
        proj: &ProjectileRenderInfo,
        item_id: i32, // TODO:
        base_state: &State,
    ) {
        let ty = proj.ty;
        let pos = proj.pos;
        let vel = proj.vel;

        let alpha = 1.0;
        /* TODO!
        if(IsOtherTeam)
        {
            Alpha = g_Config.m_ClShowOthersAlpha / 100.0f;
        }*/

        let weapon = match ty {
            WeaponType::Hammer => todo!(),
            WeaponType::Gun => pipe.weapon_container.get_or_default(
                "TODO:",
                pipe.graphics,
                pipe.fs,
                pipe.io_batcher,
            ),
            WeaponType::Shotgun => todo!(),
            WeaponType::Grenade => todo!(),
            WeaponType::Laser => todo!(),
            WeaponType::Ninja => todo!(),
            WeaponType::NumWeapons => todo!(),
            WeaponType::MaxWeapons => todo!(),
            WeaponType::Invalid => todo!(),
        };

        let mut quad_scope = quad_scope_begin();
        quad_scope.set_state(base_state);

        // add particle for this projectile
        // don't check for validity of the projectile for the current weapon here, so particle effects are rendered for mod compatibility
        if ty == WeaponType::Grenade {
            // TODO: pipe.effects.SmokeTrail(Pos, Vel * -1, Alpha);

            quad_scope.set_rotation(
                (pipe.sys.time_get_nanoseconds().as_secs_f32() as f64 * PI_F64 * 2.0 * 2.0
                    + item_id as f64/* TODO: <- ? */) as f32,
            );
        } else {
            // pipe.effects.BulletTrail(Pos, Alpha);

            if length(&vel) > 0.00001 {
                quad_scope.set_rotation(angle(&vel));
            } else {
                quad_scope.set_rotation(0.0);
            }
        }

        // TODO: if(GameClient()->m_GameSkin.m_aSpriteWeaponProjectiles[CurWeapon].IsValid())
        {
            match ty {
                WeaponType::Hammer => todo!(),
                WeaponType::Gun => quad_scope.set_texture(&weapon.gun.projectiles[0]),
                WeaponType::Shotgun => todo!(),
                WeaponType::Grenade => todo!(),
                WeaponType::Laser => todo!(),
                WeaponType::Ninja => todo!(),
                WeaponType::NumWeapons => todo!(),
                WeaponType::MaxWeapons => todo!(),
                WeaponType::Invalid => todo!(),
            }
            // TODO: Graphics()->TextureSet(GameClient()->m_GameSkin.m_aSpriteWeaponProjectiles[CurWeapon]);
            quad_scope.set_colors_from_single(1.0, 1.0, 1.0, alpha);
            pipe.graphics
                .quad_container_handle
                .render_quad_container_as_sprite(
                    &self.items_quad_container,
                    self.projectile_sprite_offset,
                    pos.x,
                    pos.y,
                    1.0,
                    1.0,
                    quad_scope,
                );
            // Graphics()->RenderQuadContainerAsSprite(m_ItemsQuadContainerIndex, m_aProjectileOffset[CurWeapon], Pos.x, Pos.y);
        }
    }

    pub fn render_pickup(
        &mut self,
        pipe: &mut RenderPipe,
        pickup: &PickupRenderInfo,
        base_state: &State,
    ) {
        let ty = pickup.ty;
        let angle = 0.0;

        let mut pos = pickup.pos;

        let pickup_tex = pipe.pickup_container.get_or_default(
            "default",
            pipe.graphics,
            pipe.fs,
            pipe.io_batcher,
        );

        let mut quad_scope = quad_scope_begin();
        quad_scope.set_state(base_state);
        match ty {
            PickupType::PowerupHealth => quad_scope.set_texture(&pickup_tex.health),
            PickupType::PowerupArmor => quad_scope.set_texture(&pickup_tex.armor),
        }
        /* TODO:
        else if(pCurrent.m_Type == POWERUP_WEAPON)
        {
            QuadOffset = m_aPickupWeaponOffset[CurWeapon];
            Graphics()->TextureSet(GameClient()->m_GameSkin.m_aSpritePickupWeapons[CurWeapon]);
        }
        else if(pCurrent.m_Type == POWERUP_NINJA)
        {
            QuadOffset = m_PickupNinjaOffset;
            m_pClient->m_Effects.PowerupShine(Pos, vec2(96, 18));
            Pos.x -= 10.0f;
            Graphics()->TextureSet(GameClient()->m_GameSkin.m_SpritePickupNinja);
        }
        else if(pCurrent.m_Type >= POWERUP_ARMOR_SHOTGUN && pCurrent.m_Type <= POWERUP_ARMOR_LASER)
        {
            QuadOffset = m_aPickupWeaponArmorOffset[pCurrent.m_Type - POWERUP_ARMOR_SHOTGUN];
            Graphics()->TextureSet(GameClient()->m_GameSkin.m_aSpritePickupWeaponArmor[pCurrent.m_Type - POWERUP_ARMOR_SHOTGUN]);
        }*/
        quad_scope.set_colors_from_single(1.0, 1.0, 1.0, 1.0);
        quad_scope.set_rotation(angle);

        let offset = pos.y / 32.0 + pos.x / 32.0;
        let cur_time_f = pipe.sys.time_get_nanoseconds().as_secs_f32();
        pos.x += (cur_time_f * 2.0 + offset).cos() * 2.5;
        pos.y += (cur_time_f * 2.0 + offset).sin() * 2.5;

        pipe.graphics
            .quad_container_handle
            .render_quad_container_as_sprite(
                &self.items_quad_container,
                self.pickup_sprite_off,
                pos.x,
                pos.y,
                1.0,
                1.0,
                quad_scope,
            );
    }

    pub fn render_flag(
        &mut self,
        pipe: &mut RenderPipe,
        flag: &FlagRenderInfo,
        base_state: &State,
    ) {
        let angle = 0.0;
        let size = 42.0;

        let ctf_tex =
            pipe.ctf_container
                .get_or_default("default", pipe.graphics, pipe.fs, pipe.io_batcher);

        let mut quad_scope = quad_scope_begin();
        quad_scope.set_state(base_state);
        if false
        // TODO: pCurrent.m_Team == TEAM_RED
        {
            quad_scope.set_texture(&ctf_tex.flag_red);
        } else {
            quad_scope.set_texture(&ctf_tex.flag_blue);
        }
        quad_scope.set_colors_from_single(1.0, 1.0, 1.0, 1.0);

        quad_scope.set_rotation(angle);

        let pos = flag.pos;

        /* TODO: if(pCurGameData)
        {
            int FlagCarrier = (pCurrent.m_Team == TEAM_RED) ? pCurGameData->m_FlagCarrierRed : pCurGameData->m_FlagCarrierBlue;
            // use the flagcarriers position if available
            if(FlagCarrier >= 0 && m_pClient->m_Snap.m_aCharacters[FlagCarrier].m_Active)
                Pos = m_pClient->m_aClients[FlagCarrier].m_RenderPos;

            // make sure that the flag isn't interpolated between capture and return
            if(pPrevGameData &&
                ((pCurrent.m_Team == TEAM_RED && pPrevGameData->m_FlagCarrierRed != pCurGameData->m_FlagCarrierRed) ||
                    (pCurrent.m_Team == TEAM_BLUE && pPrevGameData->m_FlagCarrierBlue != pCurGameData->m_FlagCarrierBlue)))
                Pos = vec2(pCurrent.m_X, pCurrent.m_Y);
        }*/

        pipe.graphics
            .quad_container_handle
            .render_quad_container_as_sprite(
                &self.items_quad_container,
                self.pickup_sprite_off,
                pos.x,
                pos.y - size * 0.75,
                1.0,
                1.0,
                quad_scope,
            );
    }

    pub fn render_laser(
        &mut self,
        pipe: &mut RenderPipe,
        cur: &LaserRenderInfo,
        base_state: &State,
    ) {
        let rgb: ColorRGBA = Default::default();
        let from = cur.from;
        let pos = cur.pos;
        let laser_len = distance(&pos, &from);

        let _color_in = 0; // TODO
        let _color_out = 0; // TODO

        /* TODO: let ty = cur.ty; match ty
        {
        LaserType::Rifle => {
            ColorOut = g_Config.m_ClLaserRifleOutlineColor;
            ColorIn = g_Config.m_ClLaserRifleInnerColor;
        }
            LaserType::Shotgun=> {
            ColorOut = g_Config.m_ClLaserShotgunOutlineColor;
            ColorIn = g_Config.m_ClLaserShotgunInnerColor;
        }
            LaserType::Door => {
            ColorOut = g_Config.m_ClLaserDoorOutlineColor;
            ColorIn = g_Config.m_ClLaserDoorInnerColor;
        }
            LaserType::Freeze => {
            ColorOut = g_Config.m_ClLaserFreezeOutlineColor;
            ColorIn = g_Config.m_ClLaserFreezeInnerColor;
            }
        }*/

        // TODO: RGB = color_cast<ColorRGBA>(ColorHSLA(ColorOut));
        let outer_color = ColorRGBA::new(rgb.r, rgb.g, rgb.b, 1.0);
        // TODO: RGB = color_cast<ColorRGBA>(ColorHSLA(ColorIn));
        let inner_color = ColorRGBA::new(rgb.r, rgb.g, rgb.b, 1.0);

        // TODO: int TuneZone = GameClient()->m_GameWorld.m_WorldConfig.m_UseTuneZones ? Collision()->IsTune(Collision()->GetMapIndex(From)) : 0;
        // TODO: let IsOtherTeam = (pCurrent.m_ExtraInfo && pCurrent.m_Owner >= 0 && m_pClient->IsOtherTeam(pCurrent.m_Owner));
        let is_other_team = false;

        let mut alpha = 1.0;
        if is_other_team {
            alpha = 1.0; // TODO: g_Config.m_ClShowOthersAlpha / 100.0f;
        }

        if laser_len > 0.0 {
            let dir = normalize_pre_length(&(pos - from), laser_len);

            let ticks_per_second = pipe.map.game.game_tick_speed();
            let intra_tick_ratio =
                intra_tick_time_to_ratio(pipe.client_data.intra_tick_time, ticks_per_second);
            let ticks = (pipe.cur_tick - cur.start_tick) as f32 + intra_tick_ratio as f32;

            let ms = (ticks / ticks_per_second as f32) * 1000.0;
            let mut a = ms / 1.0; // TODO: m_pClient->GetTunes(TuneZone).m_LaserBounceDelay;
            a = a.clamp(0.0, 1.0);
            let ia = 1.0 - a;

            // do outline
            let mut quad_scope = quad_scope_begin();
            quad_scope.set_state(base_state);
            quad_scope.set_colors_from_single(outer_color.r, outer_color.g, outer_color.b, alpha);
            let mut out = vec2::new(dir.y, -dir.x) * (7.0 * ia);

            /* TODO: IGraphics::CFreeformItem Freeform(
                From.x - Out.x, From.y - Out.y,
                From.x + Out.x, From.y + Out.y,
                Pos.x - Out.x, Pos.y - Out.y,
                Pos.x + Out.x, Pos.y + Out.y);
            Graphics()->QuadsDrawFreeform(&Freeform, 1);*/

            // do inner
            out = vec2::new(dir.y, -dir.x) * (5.0 * ia);
            quad_scope.set_colors_from_single(inner_color.r, inner_color.g, inner_color.b, alpha);
            // center

            /* TODO: Freeform = IGraphics::CFreeformItem(
                From.x - Out.x, From.y - Out.y,
                From.x + Out.x, From.y + Out.y,
                Pos.x - Out.x, Pos.y - Out.y,
                Pos.x + Out.x, Pos.y + Out.y);
            Graphics()->QuadsDrawFreeform(&Freeform, 1);*/
        }

        // render head
        {
            // TODO: let CurParticle = pipe.cur_tick % 3;
            let mut quad_scope = quad_scope_begin();
            // TODO: Graphics()->TextureSet(GameClient()->m_ParticlesSkin.m_aSpriteParticleSplat[CurParticle]);
            quad_scope.set_rotation(pipe.cur_tick as f32);
            quad_scope.set_colors_from_single(outer_color.r, outer_color.g, outer_color.b, alpha);
            pipe.graphics
                .quad_container_handle
                .render_quad_container_as_sprite(
                    &self.items_quad_container,
                    self.particle_splat_off,
                    pos.x,
                    pos.y,
                    1.0,
                    1.0,
                    quad_scope,
                );
            // inner
            let mut quad_scope = quad_scope_begin();
            // TODO: Graphics()->TextureSet(GameClient()->m_ParticlesSkin.m_aSpriteParticleSplat[CurParticle]);
            quad_scope.set_rotation(pipe.cur_tick as f32);
            quad_scope.set_colors_from_single(inner_color.r, inner_color.g, inner_color.b, alpha);
            pipe.graphics
                .quad_container_handle
                .render_quad_container_as_sprite(
                    &self.items_quad_container,
                    self.particle_splat_off,
                    pos.x,
                    pos.y,
                    20.0 / 24.0,
                    20.0 / 24.0,
                    quad_scope,
                );
        }
    }
}
