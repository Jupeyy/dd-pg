use std::time::Duration;

use client_containers::{
    ctf::CtfContainer, game::GameContainer, ninja::NinjaContainer, weapons::WeaponContainer,
};
use client_render_base::{
    map::render_pipe::{Camera, GameTimeInfo},
    render::{
        canvas_mapping::CanvasMappingIngame,
        effects::Effects,
        particle_manager::ParticleManager,
        toolkit::{get_ninja_as_quad, get_weapon_as_quad, pickup_scale},
    },
};
use game_interface::types::{
    emoticons::EnumCount,
    flag::FlagType,
    game::GameEntityId,
    pickup::PickupType,
    render::{
        character::CharacterInfo, flag::FlagRenderInfo, laser::LaserRenderInfo,
        pickup::PickupRenderInfo, projectiles::ProjectileRenderInfo,
    },
    weapons::WeaponType,
};
use graphics::{
    graphics::graphics::Graphics,
    handles::{
        quad_container::quad_container::QuadContainer,
        stream::stream::{GraphicsStreamHandle, QuadStreamHandle},
        stream_types::StreamedQuad,
    },
    quad_container::Quad,
    streaming::quad_scope_begin,
};
use graphics_types::rendering::{ColorRgba, State};
use hiarc::hi_closure;
use math::math::{
    angle, distance, length, normalize_pre_length,
    vector::{ubvec4, vec2, vec4},
    PI_F64,
};
use num_traits::FromPrimitive;
use pool::datatypes::PoolLinkedHashMap;
use shared_base::game_types::intra_tick_time_to_ratio;

pub struct GameObjectsRender {
    cur_time: Duration,

    items_quad_container: QuadContainer,
    canvas_mapping: CanvasMappingIngame,
    stream_handle: GraphicsStreamHandle,

    // offsets
    ctf_flag_offset: usize, // TODO
    projectile_sprite_offset: usize,
    pickup_sprite_off: usize,
    particle_splat_off: usize,

    weapon_quad_offsets: [usize; WeaponType::COUNT],
    ninja_quad_offset: usize,
}

pub struct GameObjectsRenderPipe<'a> {
    pub particle_manager: &'a mut ParticleManager,
    pub cur_time: &'a Duration,

    pub game_time_info: &'a GameTimeInfo,
    pub character_infos: &'a PoolLinkedHashMap<GameEntityId, CharacterInfo>,
    pub projectiles: &'a PoolLinkedHashMap<GameEntityId, ProjectileRenderInfo>,
    pub flags: &'a PoolLinkedHashMap<GameEntityId, FlagRenderInfo>,
    pub lasers: &'a PoolLinkedHashMap<GameEntityId, LaserRenderInfo>,
    pub pickups: &'a PoolLinkedHashMap<GameEntityId, PickupRenderInfo>,

    pub ctf_container: &'a mut CtfContainer,
    pub game_container: &'a mut GameContainer,
    pub ninja_container: &'a mut NinjaContainer,
    pub weapon_container: &'a mut WeaponContainer,

    pub local_character_id: Option<&'a GameEntityId>,

    pub camera: &'a Camera,
}

impl GameObjectsRender {
    pub fn new(cur_time: &Duration, graphics: &Graphics) -> Self {
        let mut quads: Vec<Quad> = Default::default();

        let quad = Quad::new()
            .from_rect(-21.0 / 32.0, -42.0 / 32.0, 42.0 / 32.0, 84.0 / 32.0)
            .with_color(&ubvec4::new(255, 255, 255, 255))
            .with_uv_from_points(&vec2::new(0.0, 0.0), &vec2::new(1.0, 1.0));

        let ctf_flag_offset = quads.len();
        quads.push(quad);

        let sprite_scale = pickup_scale();
        let quad = Quad::new()
            .from_width_and_height_centered(2.0 * sprite_scale.0, 2.0 * sprite_scale.1)
            .with_color(&ubvec4::new(255, 255, 255, 255))
            .with_uv_from_points(&vec2::new(0.0, 0.0), &vec2::new(1.0, 1.0));

        let pickup_sprite_off = quads.len();
        quads.push(quad);

        let mut weapon_quad_offsets: [usize; WeaponType::COUNT] = Default::default();

        (0..WeaponType::COUNT).enumerate().for_each(|(index, wi)| {
            let quad = get_weapon_as_quad(&FromPrimitive::from_usize(wi).unwrap())
                .with_color(&ubvec4::new(255, 255, 255, 255))
                .with_uv_from_points(&vec2::new(0.0, 0.0), &vec2::new(1.0, 1.0));
            let offset_normal = quads.len();
            quads.push(quad);
            weapon_quad_offsets[index] = offset_normal;
        });

        let quad = get_ninja_as_quad(true)
            .with_color(&ubvec4::new(255, 255, 255, 255))
            .with_uv_from_points(&vec2::new(0.0, 0.0), &vec2::new(1.0, 1.0));
        let offset_normal = quads.len();
        quads.push(quad);
        let ninja_quad_offset = offset_normal;

        let quad = Quad::new()
            .from_width_and_height_centered(1.0, 1.0)
            .with_color(&ubvec4::new(255, 255, 255, 255))
            .with_uv_from_points(&vec2::new(0.0, 0.0), &vec2::new(1.0, 1.0));

        let projectile_sprite_off = quads.len();
        quads.push(quad);

        let quad = Quad::new()
            .from_width_and_height_centered(0.75, 0.75)
            .with_color(&ubvec4::new(255, 255, 255, 255))
            .with_uv_from_points(&vec2::new(0.0, 0.0), &vec2::new(1.0, 1.0));

        let particle_splat_off = quads.len();
        quads.push(quad);

        let items_quad_container = graphics.quad_container_handle.create_quad_container(quads);

        Self {
            cur_time: *cur_time,
            items_quad_container,
            canvas_mapping: CanvasMappingIngame::new(graphics),
            stream_handle: graphics.stream_handle.clone(),

            ctf_flag_offset,
            projectile_sprite_offset: projectile_sprite_off,
            pickup_sprite_off,
            particle_splat_off,

            weapon_quad_offsets,
            ninja_quad_offset,
        }
    }

    pub fn render(&mut self, pipe: &mut GameObjectsRenderPipe) {
        self.cur_time = *pipe.cur_time;
        let mut base_state = State::default();
        let center = pipe.camera.pos;
        self.canvas_mapping.map_canvas_for_ingame_items(
            &mut base_state,
            center.x,
            center.y,
            pipe.camera.zoom,
        );

        pipe.projectiles.values().for_each(|proj| {
            self.render_projectile(pipe, proj, pipe.character_infos, &base_state);
        });
        pipe.flags.values().for_each(|flag| {
            self.render_flag(pipe, flag, pipe.character_infos, &base_state);
        });
        pipe.lasers.values().for_each(|laser| {
            self.render_laser(pipe, laser, pipe.character_infos, &base_state);
        });
        pipe.pickups.values().for_each(|pickup| {
            self.render_pickup(pipe, pickup, &base_state);
        });
    }

    pub fn render_projectile(
        &mut self,
        pipe: &mut GameObjectsRenderPipe,
        proj: &ProjectileRenderInfo,
        character_infos: &PoolLinkedHashMap<GameEntityId, CharacterInfo>,
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

        let weapon_name = proj
            .owner_id
            .and_then(|id| character_infos.get(&id))
            .map(|c| &c.info.weapon);
        let weapon = pipe.weapon_container.get_or_default_opt(weapon_name);

        let mut quad_scope = quad_scope_begin();
        quad_scope.set_state(base_state);

        // add particle for this projectile
        // don't check for validity of the projectile for the current weapon here, so particle effects are rendered for mod compatibility
        if ty == WeaponType::Grenade {
            let mut effects = Effects::new(pipe.particle_manager, self.cur_time);
            effects.smoke_trail(&pos, &(vel * -1.0), alpha, 0.0);

            quad_scope
                .set_rotation((pipe.cur_time.as_secs_f32() as f64 * PI_F64 * 2.0 * 2.0) as f32);
        } else {
            let mut effects = Effects::new(pipe.particle_manager, self.cur_time);
            effects.bullet_trail(&pos, alpha);

            if length(&vel) > 0.00001 {
                quad_scope.set_rotation(angle(&vel));
            } else {
                quad_scope.set_rotation(0.0);
            }
        }

        let texture = match ty {
            WeaponType::Hammer => panic!("hammers have no projectiles"),
            WeaponType::Gun => &weapon.gun.projectiles[0],
            WeaponType::Shotgun => &weapon.shotgun.weapon.projectiles[0],
            WeaponType::Grenade => &weapon.grenade.weapon.projectiles[0],
            WeaponType::Laser => panic!("lasers have no projectiles"),
        };
        quad_scope.set_colors_from_single(1.0, 1.0, 1.0, alpha);
        self.items_quad_container.render_quad_container_as_sprite(
            self.projectile_sprite_offset,
            pos.x,
            pos.y,
            1.0,
            1.0,
            quad_scope,
            texture.into(),
        );
    }

    pub fn render_pickup(
        &mut self,
        pipe: &mut GameObjectsRenderPipe,
        pickup: &PickupRenderInfo,
        base_state: &State,
    ) {
        let ty = pickup.ty;
        let angle = 0.0;

        let mut pos = pickup.pos;

        let mut quad_scope = quad_scope_begin();
        quad_scope.set_state(base_state);
        let (texture, quad_offset) = match ty {
            PickupType::PowerupHealth => {
                let key = pickup
                    .owner_id
                    .and_then(|id| pipe.character_infos.get(&id))
                    .or_else(|| {
                        pipe.local_character_id
                            .and_then(|id| pipe.character_infos.get(id))
                    })
                    .map(|c| &c.info.game);

                (
                    &pipe.game_container.get_or_default_opt(key).heart.tex,
                    self.pickup_sprite_off,
                )
            }
            PickupType::PowerupArmor => {
                let key = pickup
                    .owner_id
                    .and_then(|id| pipe.character_infos.get(&id))
                    .or_else(|| {
                        pipe.local_character_id
                            .and_then(|id| pipe.character_infos.get(id))
                    })
                    .map(|c| &c.info.game);
                (
                    &pipe.game_container.get_or_default_opt(key).shield.tex,
                    self.pickup_sprite_off,
                )
            }
            PickupType::PowerupWeapon(weapon) => {
                let key = pickup
                    .owner_id
                    .and_then(|id| pipe.character_infos.get(&id))
                    .or_else(|| {
                        pipe.local_character_id
                            .and_then(|id| pipe.character_infos.get(id))
                    })
                    .map(|c| &c.info.weapon);
                // go by weapon type instead
                let weapon_tex = pipe.weapon_container.get_or_default_opt(key);
                (
                    &weapon_tex.by_type(weapon).tex,
                    self.weapon_quad_offsets[weapon as usize],
                )
            }
            PickupType::PowerupNinja => {
                // randomly move the pickup a bit to the left
                pos.x -= 10.0 / 32.0;
                Effects::new(pipe.particle_manager, self.cur_time)
                    .powerup_shine(&pos, &vec2::new(3.0, 18.0 / 32.0));

                let key = pickup
                    .owner_id
                    .and_then(|id| pipe.character_infos.get(&id))
                    .or_else(|| {
                        pipe.local_character_id
                            .and_then(|id| pipe.character_infos.get(id))
                    })
                    .map(|c| &c.info.ninja);
                (
                    &pipe.ninja_container.get_or_default_opt(key).weapon, // TODO:
                    self.ninja_quad_offset,
                )
            }
        };
        /* TODO:
        else if(pCurrent.m_Type >= POWERUP_ARMOR_SHOTGUN && pCurrent.m_Type <= POWERUP_ARMOR_LASER)
        {
            QuadOffset = m_aPickupWeaponArmorOffset[pCurrent.m_Type - POWERUP_ARMOR_SHOTGUN];
            Graphics()->TextureSet(GameClient()->m_GameSkin.m_aSpritePickupWeaponArmor[pCurrent.m_Type - POWERUP_ARMOR_SHOTGUN]);
        }*/
        quad_scope.set_colors_from_single(1.0, 1.0, 1.0, 1.0);
        quad_scope.set_rotation(angle);

        let offset = pos.y + pos.x;
        let cur_time_f = pipe.cur_time.as_secs_f32();
        pos.x += (cur_time_f * 2.0 + offset).cos() * 2.5 / 32.0;
        pos.y += (cur_time_f * 2.0 + offset).sin() * 2.5 / 32.0;

        self.items_quad_container.render_quad_container_as_sprite(
            quad_offset,
            pos.x,
            pos.y,
            1.0,
            1.0,
            quad_scope,
            texture.into(),
        );
    }

    pub fn render_flag(
        &mut self,
        pipe: &mut GameObjectsRenderPipe,
        flag: &FlagRenderInfo,
        character_infos: &PoolLinkedHashMap<GameEntityId, CharacterInfo>,
        base_state: &State,
    ) {
        let angle = 0.0;
        let size = 42.0 / 32.0;
        let ty = flag.ty;

        let key = flag
            .owner_id
            .and_then(|id| character_infos.get(&id))
            .or_else(|| {
                pipe.local_character_id
                    .and_then(|id| pipe.character_infos.get(id))
            })
            .map(|c| &c.info.ctf);
        let ctf_tex = pipe.ctf_container.get_or_default_opt(key);

        let mut quad_scope = quad_scope_begin();
        quad_scope.set_state(base_state);
        let texture = if let FlagType::Red = ty {
            &ctf_tex.flag_red
        } else {
            &ctf_tex.flag_blue
        };
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

        self.items_quad_container.render_quad_container_as_sprite(
            self.ctf_flag_offset,
            pos.x,
            pos.y - size * 0.75,
            1.0,
            1.0,
            quad_scope,
            texture.into(),
        );
    }

    pub fn render_laser(
        &mut self,
        pipe: &mut GameObjectsRenderPipe,
        cur: &LaserRenderInfo,
        character_infos: &PoolLinkedHashMap<GameEntityId, CharacterInfo>,
        base_state: &State,
    ) {
        let rgb: ColorRgba = ColorRgba {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
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
        let outer_color = ColorRgba::new(rgb.r, rgb.g, rgb.b, 1.0);
        // TODO: RGB = color_cast<ColorRGBA>(ColorHSLA(ColorIn));
        let inner_color = ColorRgba::new(rgb.r, rgb.g, rgb.b, 1.0);

        // TODO: int TuneZone = GameClient()->m_GameWorld.m_WorldConfig.m_UseTuneZones ? Collision()->IsTune(Collision()->GetMapIndex(From)) : 0;
        // TODO: let IsOtherTeam = (pCurrent.m_ExtraInfo && pCurrent.m_Owner >= 0 && m_pClient->IsOtherTeam(pCurrent.m_Owner));
        let is_other_team = false;

        let mut alpha = 1.0;
        if is_other_team {
            alpha = 1.0; // TODO: g_Config.m_ClShowOthersAlpha / 100.0f;
        }

        if laser_len > 0.0 {
            let dir = normalize_pre_length(&(pos - from), laser_len);

            let ticks_per_second = pipe.game_time_info.ticks_per_second;
            let intra_tick_ratio =
                intra_tick_time_to_ratio(pipe.game_time_info.intra_tick_time, ticks_per_second);
            let ticks = cur.eval_tick_ratio.map(|(eval_tick, lifetime)| {
                (eval_tick as f64 + intra_tick_ratio) / lifetime.get() as f64
            });

            let ms = ticks.unwrap_or(1.0) as f32;
            let mut a = ms;
            a = a.clamp(0.0, 1.0);
            let ia = 1.0 - a;

            // do outline
            self.stream_handle.render_quads(
                hi_closure!([from: vec2, pos: vec2, dir: vec2, outer_color: ColorRgba, inner_color: ColorRgba, alpha: f32, ia: f32], |mut stream_handle: QuadStreamHandle<'_>| -> () {
                    let out = vec2::new(dir.y, -dir.x) * (7.0 / 32.0 * ia);
                    stream_handle.add_vertices(
                        StreamedQuad::default()
                            .pos_free_form(
                                vec2::new(from.x - out.x, from.y - out.y),
                                vec2::new(from.x + out.x, from.y + out.y),
                                vec2::new(pos.x - out.x, pos.y - out.y),
                                vec2::new(pos.x + out.x, pos.y + out.y),
                            )
                            .colorf(vec4::new(
                                outer_color.r,
                                outer_color.g,
                                outer_color.b,
                                alpha,
                            ))
                            .into(),
                    );

                    // do inner
                    let out = vec2::new(dir.y, -dir.x) * (5.0 / 32.0 * ia);
                    stream_handle.add_vertices(
                        StreamedQuad::default()
                            .pos_free_form(
                                vec2::new(from.x - out.x, from.y - out.y),
                                vec2::new(from.x + out.x, from.y + out.y),
                                vec2::new(pos.x - out.x, pos.y - out.y),
                                vec2::new(pos.x + out.x, pos.y + out.y),
                            )
                            .colorf(vec4::new(
                                inner_color.r,
                                inner_color.g,
                                inner_color.b,
                                alpha,
                            ))
                            .into(),
                    );
                }),
                *base_state,
            );
        }

        // render head
        let key = cur
            .owner_id
            .and_then(|id| character_infos.get(&id))
            .map(|c| &c.info.weapon);
        let heads = &pipe.weapon_container.get_or_default_opt(key).laser.heads;
        {
            let head_index = pipe.particle_manager.rng.random_int_in(0..=2) as usize;
            let mut quad_scope = quad_scope_begin();
            quad_scope.set_state(base_state);
            quad_scope.set_rotation(
                (pipe.cur_time.as_secs_f64() * pipe.game_time_info.ticks_per_second.get() as f64)
                    .rem_euclid(pipe.game_time_info.ticks_per_second.get() as f64)
                    as f32,
            );
            quad_scope.set_colors_from_single(outer_color.r, outer_color.g, outer_color.b, alpha);
            self.items_quad_container.render_quad_container_as_sprite(
                self.particle_splat_off,
                pos.x,
                pos.y,
                1.0,
                1.0,
                quad_scope,
                (&heads[head_index]).into(),
            );
            // inner
            let mut quad_scope = quad_scope_begin();
            quad_scope.set_state(base_state);
            quad_scope.set_rotation(
                (pipe.cur_time.as_secs_f64() * pipe.game_time_info.ticks_per_second.get() as f64)
                    .rem_euclid(pipe.game_time_info.ticks_per_second.get() as f64)
                    as f32,
            );
            quad_scope.set_colors_from_single(inner_color.r, inner_color.g, inner_color.b, alpha);
            self.items_quad_container.render_quad_container_as_sprite(
                self.particle_splat_off,
                pos.x,
                pos.y,
                20.0 / 24.0,
                20.0 / 24.0,
                quad_scope,
                (&heads[head_index]).into(),
            );
        }
    }
}
