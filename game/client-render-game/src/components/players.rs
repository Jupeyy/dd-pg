use std::{collections::HashMap, time::Duration};

use client_containers_new::{
    emoticons::EmoticonsContainer,
    hooks::{Hook, HookContainer},
    ninja::NinjaContainer,
    skins::SkinContainer,
    weapons::WeaponContainer,
};
use client_render::{
    emoticons::render::{RenderEmoticon, RenderEmoticonPipe},
    nameplates::render::{NameplateRender, NameplateRenderPipe},
};
use client_render_base::{
    map::render_pipe::{Camera, GameStateRenderInfo},
    render::{
        animation::{AnimState, TeeAnimationFrame},
        canvas_mapping::CanvasMappingIngame,
        default_anim::{
            base_anim, hammer_swing_anim, idle_anim, inair_anim, ninja_swing_anim, run_left_anim,
            run_right_anim, walk_anim,
        },
        tee::{
            RenderTee, TeeRenderHand, TeeRenderHands, TeeRenderInfo, TeeRenderSkinTextures,
            RENDER_TEE_SIZE,
        },
        weapons::WeaponNinjaSpec,
    },
};
use graphics::{
    graphics::graphics::Graphics,
    handles::{
        quad_container::quad_container::QuadContainer,
        stream::stream::{GraphicsStreamHandle, StreamedSprites},
    },
    quad_container::Quad,
    streaming::quad_scope_begin,
};

use graphics_types::{
    commands::RenderSpriteInfo,
    rendering::{ColorRGBA, State},
};
use hiarc::hi_closure;
use num_traits::FromPrimitive;
use pool::datatypes::PoolLinkedHashMap;
use shared_base::game_types::intra_tick_time_to_ratio;

use client_render_base::render::weapons::{
    WeaponGrenadeSpec, WeaponGunSpec, WeaponHammerSpec, WeaponLaserSpec, WeaponShotgunSpec,
};

use shared_game::{
    collision::collision::Collision,
    weapons::definitions::weapon_def::{
        get_ninja_sprite_scale, get_scale, get_weapon_sprite_scale, get_weapon_visual_scale,
        NINJA_PICKUP_VISUAL_SIZE, NINJA_WEAPON_VISUAL_SIZE,
    },
};

use game_interface::types::{
    emoticons::EmoticonType,
    game::{GameEntityId, GameTickType},
    render::character::{CharacterBuff, CharacterInfo, CharacterRenderInfo},
    weapons::{WeaponType, NUM_WEAPONS},
};
use math::math::{
    angle, distance, mix, normalize,
    vector::{dvec2, ubvec4, vec2},
    Rng, RngSlice, PI, PI_F64,
};

use super::{
    effects::Effects, game_objects::get_sprite_scale_impl, particle_manager::ParticleManager,
};

pub struct PlayerRenderPipe<'a> {
    pub cur_time: &'a Duration,
    pub game_info: &'a GameStateRenderInfo,
    pub render_infos: &'a PoolLinkedHashMap<GameEntityId, CharacterRenderInfo>,
    pub character_infos: &'a PoolLinkedHashMap<GameEntityId, CharacterInfo>,

    pub skins: &'a mut SkinContainer,
    pub ninjas: &'a mut NinjaContainer,
    pub hooks: &'a mut HookContainer,
    pub weapons: &'a mut WeaponContainer,
    pub emoticons: &'a mut EmoticonsContainer,

    pub particle_manager: &'a mut ParticleManager,

    pub collision: &'a Collision,
    pub camera: &'a Camera,
}

/// The player component renders all hooks
/// all weapons, and all players
pub struct Players {
    quad_container: QuadContainer,
    canvas_mapping: CanvasMappingIngame,
    stream_handle: GraphicsStreamHandle,

    pub tee_renderer: RenderTee,
    nameplate_renderer: NameplateRender,
    emoticon_renderer: RenderEmoticon,

    hook_chain_quad_offset: usize,
    hook_head_quad_offset: usize,

    weapon_quad_offsets: HashMap<WeaponType, (usize, usize)>,
    weapon_muzzle_quad_offsets: HashMap<WeaponType, (usize, usize)>,

    ninja_quad_offsets: (usize, usize),
    ninja_muzzle_quad_offset: usize,

    rng: Rng,
}

pub fn get_weapon_as_quad(weapon: &WeaponType) -> Quad {
    let size = get_weapon_visual_scale(weapon);
    let scale = get_weapon_sprite_scale(weapon);
    Quad::new().from_width_and_height_centered(size * scale.0, size * scale.1)
}

pub fn get_ninja_as_quad(as_pickup: bool) -> Quad {
    let size = if as_pickup {
        NINJA_PICKUP_VISUAL_SIZE
    } else {
        NINJA_WEAPON_VISUAL_SIZE
    };
    let scale = get_ninja_sprite_scale();
    Quad::new().from_width_and_height_centered(size * scale.0, size * scale.1)
}

impl Players {
    pub fn new(graphics: &Graphics) -> Self {
        let tee_renderer = RenderTee::new(graphics);
        let nameplate_renderer = NameplateRender::new(graphics);
        let emoticon_renderer = RenderEmoticon::new(graphics);

        let mut quads: Vec<Quad> = Default::default();

        let mut weapon_quad_offsets: HashMap<WeaponType, (usize, usize)> = Default::default();

        weapon_quad_offsets.reserve(NUM_WEAPONS);
        (0..NUM_WEAPONS).enumerate().for_each(|(index, wi)| {
            let mut quad = get_weapon_as_quad(&FromPrimitive::from_usize(wi).unwrap())
                .with_color(&ubvec4::new(255, 255, 255, 255));

            quad = quad.with_uv_from_points(&vec2::new(0.0, 0.0), &vec2::new(1.0, 1.0));
            let offset_normal = quads.len();
            quads.push(quad);
            quad = quad.with_uv_from_points(&vec2::new(0.0, 1.0), &vec2::new(1.0, 0.0));
            let offset_flipped = quads.len();
            quads.push(quad);
            weapon_quad_offsets.insert(
                WeaponType::from_usize(index).unwrap(),
                (offset_normal, offset_flipped),
            );
        });

        let mut weapon_muzzle_quad_offsets: HashMap<WeaponType, (usize, usize)> =
            Default::default();

        weapon_muzzle_quad_offsets.reserve(NUM_WEAPONS);
        (0..NUM_WEAPONS).for_each(|wi| {
            let weapon = FromPrimitive::from_usize(wi).unwrap();
            let (scale_x, scale_y);
            if weapon == WeaponType::Gun || weapon == WeaponType::Shotgun {
                // TODO: hardcoded for now to get the same particle size as before
                (scale_x, scale_y) = get_sprite_scale_impl(3, 2);
            } else {
                // TODO: RenderTools()->GetSpriteScale(g_pData->m_Weapons.m_aId[i].m_aSpriteMuzzles[n], ScaleX, ScaleY);
                (scale_x, scale_y) = (0.0, 0.0);
            }
            let size = get_weapon_visual_scale(&weapon);
            let width = (size * scale_x) * (4.0 / 3.0);
            let height = size * scale_y;

            let mut quad = Quad::new()
                .from_width_and_height_centered(width, height)
                .with_color(&ubvec4::new(255, 255, 255, 255));
            let offset_normal = quads.len();
            quads.push(quad);

            quad = quad.with_tex(&[
                vec2 { x: 0.0, y: 1.0 },
                vec2 { x: 1.0, y: 1.0 },
                vec2 { x: 1.0, y: 0.0 },
                vec2 { x: 0.0, y: 0.0 },
            ]);
            let offset_flipped = quads.len();
            quads.push(quad);

            weapon_muzzle_quad_offsets.insert(weapon, (offset_normal, offset_flipped));
        });

        // # ninja
        let mut quad = get_ninja_as_quad(false).with_color(&ubvec4::new(255, 255, 255, 255));

        quad = quad.with_uv_from_points(&vec2::new(0.0, 0.0), &vec2::new(1.0, 1.0));
        let offset_normal = quads.len();
        quads.push(quad);
        quad = quad.with_uv_from_points(&vec2::new(0.0, 1.0), &vec2::new(1.0, 0.0));
        let offset_flipped = quads.len();
        quads.push(quad);
        let ninja_quad_offsets = (offset_normal, offset_flipped);

        let (scale_x, scale_y) = get_scale(7.0, 4.0);
        let offset_normal = quads.len();
        quads.push(
            Quad::new()
                .from_width_and_height_centered(5.0 * scale_x, 5.0 * scale_y)
                .with_color(&ubvec4::new(255, 255, 255, 255)),
        );
        let ninja_muzzle_quad_offset = offset_normal;

        let quad = Quad::new()
            .from_rect(-3.0 / 8.0, -0.25, 0.75, 0.5)
            .with_color(&ubvec4::new(255, 255, 255, 255))
            .with_uv_from_points(&vec2::new(0.0, 0.0), &vec2::new(1.0, 1.0));

        let hook_chain_quad_offset = quads.len();
        quads.push(quad);
        let hook_head_quad_offset = hook_chain_quad_offset;

        let quad_container = graphics.quad_container_handle.create_quad_container(quads);
        /*
        m_WeaponEmoteQuadContainerIndex = Graphics()->CreateQuadContainer(false);

        Graphics()->SetColor(1.f, 1.f, 1.f, 1.f);

        for(int i = 0; i < NUM_WEAPONS; ++i)
        {
            float ScaleX, ScaleY;
            RenderTools()->GetSpriteScale(g_pData->m_Weapons.m_aId[i].m_pSpriteBody, ScaleX, ScaleY);
            Graphics()->QuadsSetSubset(0, 0, 1, 1);
            RenderTools()->QuadContainerAddSprite(m_WeaponEmoteQuadContainerIndex,
                g_pData->m_Weapons.m_aId[i].m_VisualSize * ScaleX, g_pData->m_Weapons.m_aId[i].m_VisualSize * ScaleY);
            Graphics()->QuadsSetSubset(0, 1, 1, 0);
            RenderTools()->QuadContainerAddSprite(m_WeaponEmoteQuadContainerIndex,
                g_pData->m_Weapons.m_aId[i].m_VisualSize * ScaleX, g_pData->m_Weapons.m_aId[i].m_VisualSize * ScaleY);
        }
        float ScaleX, ScaleY;

        // at the end the hand
        Graphics()->QuadsSetSubset(0, 0, 1, 1);
        RenderTools()->QuadContainerAddSprite(m_WeaponEmoteQuadContainerIndex, 20.f);
        Graphics()->QuadsSetSubset(0, 0, 1, 1);
        RenderTools()->QuadContainerAddSprite(m_WeaponEmoteQuadContainerIndex, 20.f);

        Graphics()->QuadsSetSubset(0, 0, 1, 1);
        RenderTools()->QuadContainerAddSprite(m_WeaponEmoteQuadContainerIndex, -12.f, -8.f, 24.f, 16.f);
        Graphics()->QuadsSetSubset(0, 0, 1, 1);
        RenderTools()->QuadContainerAddSprite(m_WeaponEmoteQuadContainerIndex, -12.f, -8.f, 24.f, 16.f);

        for(int i = 0; i < NUM_EMOTICONS; ++i)
        {
            Graphics()->QuadsSetSubset(0, 0, 1, 1);
            RenderTools()->QuadContainerAddSprite(m_WeaponEmoteQuadContainerIndex, 64.f);
        }
        Graphics()->QuadContainerUpload(m_WeaponEmoteQuadContainerIndex);

        for(int i = 0; i < NUM_WEAPONS; ++i)
        {
            m_aWeaponSpriteMuzzleQuadContainerIndex[i] = Graphics()->CreateQuadContainer(false);
            for(int n = 0; n < g_pData->m_Weapons.m_aId[i].m_NumSpriteMuzzles; ++n)
            {
                if(g_pData->m_Weapons.m_aId[i].m_aSpriteMuzzles[n])
                {
                    if(i == WEAPON_GUN || i == WEAPON_SHOTGUN)
                    {
                        // TODO: hardcoded for now to get the same particle size as before
                        RenderTools()->GetSpriteScaleImpl(96, 64, ScaleX, ScaleY);
                    }
                    else
                        RenderTools()->GetSpriteScale(g_pData->m_Weapons.m_aId[i].m_aSpriteMuzzles[n], ScaleX, ScaleY);
                }

                float SWidth = (g_pData->m_Weapons.m_aId[i].m_VisualSize * ScaleX) * (4.0f / 3.0f);
                float SHeight = g_pData->m_Weapons.m_aId[i].m_VisualSize * ScaleY;

                Graphics()->QuadsSetSubset(0, 0, 1, 1);
                if(WEAPON_NINJA == i)
                    RenderTools()->QuadContainerAddSprite(m_aWeaponSpriteMuzzleQuadContainerIndex[i], 160.f * ScaleX, 160.f * ScaleY);
                else
                    RenderTools()->QuadContainerAddSprite(m_aWeaponSpriteMuzzleQuadContainerIndex[i], SWidth, SHeight);

                Graphics()->QuadsSetSubset(0, 1, 1, 0);
                if(WEAPON_NINJA == i)
                    RenderTools()->QuadContainerAddSprite(m_aWeaponSpriteMuzzleQuadContainerIndex[i], 160.f * ScaleX, 160.f * ScaleY);
                else
                    RenderTools()->QuadContainerAddSprite(m_aWeaponSpriteMuzzleQuadContainerIndex[i], SWidth, SHeight);
            }
            Graphics()->QuadContainerUpload(m_aWeaponSpriteMuzzleQuadContainerIndex[i]);
        }

        Graphics()->QuadsSetSubset(0.f, 0.f, 1.f, 1.f);
        Graphics()->QuadsSetRotation(0.f);
         */
        Self {
            quad_container,
            canvas_mapping: CanvasMappingIngame::new(graphics),
            stream_handle: graphics.stream_handle.clone(),

            tee_renderer,
            nameplate_renderer,
            emoticon_renderer,

            hook_chain_quad_offset,
            hook_head_quad_offset,

            weapon_quad_offsets,
            weapon_muzzle_quad_offsets,

            ninja_quad_offsets,
            ninja_muzzle_quad_offset,

            rng: Rng::new(0),
        }
    }

    fn get_player_target_angle(cursor_vec: &dvec2) -> f64 {
        if cursor_vec.x == 0.0 && cursor_vec.y == 0.0 {
            return 0.0;
        } else if cursor_vec.x == 0.0 {
            return if cursor_vec.y < 0.0 {
                -PI_F64 / 2.0
            } else {
                PI_F64 / 2.0
            };
        }
        let tmp_angle = (cursor_vec.y / cursor_vec.x).atan();
        if cursor_vec.x < 0.0 {
            tmp_angle + PI_F64
        } else {
            tmp_angle
        }
    }

    fn render_hook(
        &mut self,
        hooks: &mut HookContainer,
        pos: vec2,
        player_render_info: &CharacterRenderInfo,
        base_state: State,
    ) -> Option<TeeRenderHand> {
        if let Some(hook_pos) = player_render_info.lerped_hook_pos {
            let hook_dir = normalize(&(pos - hook_pos));

            // current hook
            let cur_hook = hooks.get_or_default(&"TODO".try_into().unwrap());

            // render head
            let mut quad_scope = quad_scope_begin();
            quad_scope.set_state(&base_state);
            let texture = &cur_hook.hook_head;
            quad_scope.set_rotation(angle(&hook_dir) + PI);
            quad_scope.set_colors_from_single(1.0, 1.0, 1.0, 1.0); //<-- alpha

            let hook_render_pos = hook_pos;
            self.quad_container.render_quad_container_as_sprite(
                self.hook_head_quad_offset,
                hook_render_pos.x,
                hook_render_pos.y,
                1.0,
                1.0,
                quad_scope,
                texture.into(),
            );

            // render chain
            let quad_container = &self.quad_container;
            let hook_chain_quad_offset = self.hook_chain_quad_offset;
            self.stream_handle.fill_sprites_uniform_instance(
                hi_closure!([pos: vec2, hook_pos: vec2, hook_render_pos: vec2, hook_dir: vec2], |mut stream_handle: StreamedSprites<'_>| -> () {
                    let mut f = 0.75;
                    let d = distance(&pos, &hook_pos);
                    while f < d && f < 0.75 * 512.0 {
                        let p = hook_render_pos + hook_dir * f;
                        stream_handle.add(RenderSpriteInfo {
                            pos: p,
                            scale: 1.0,
                            rotation: angle(&hook_dir) + PI,
                        });
                        f += 0.75;
                    }
                }),
                hi_closure!([quad_container: &QuadContainer, hook_chain_quad_offset: usize, cur_hook: &Hook, base_state: State], |instance: usize, count: usize| -> () {
                    let mut quad_scope = quad_scope_begin();
                    quad_scope.set_state(&base_state);
                    quad_scope.set_colors_from_single(1.0, 1.0, 1.0, 1.0);
                    quad_container
                        .render_quad_container_as_sprite_multiple(
                            hook_chain_quad_offset,
                            instance,
                            count,
                            quad_scope,
                            (&cur_hook.hook_chain).into(),
                        );
                }),
            );
            Some(TeeRenderHand {
                pos: vec2::new(0.0, 0.0),
                dir: -hook_dir,
                rot_offset: -PI / 2.0,
                offset_after_rot: vec2::new(20.0 / 32.0, 0.0),
                scale: 1.0,
            })
        } else {
            None
        }
    }

    fn render_weapon(
        &mut self,
        weapons: &mut WeaponContainer,
        player_render_info: &CharacterRenderInfo,
        render_pos: vec2,
        ticks_in_a_second: GameTickType,
        game_info: &GameStateRenderInfo,
        base_state: State,
        is_sit: bool,
        inactive: bool,
    ) -> Option<TeeRenderHand> {
        let weapon_texture = weapons.get_or_default(&"TODO".try_into().unwrap());
        let current_weapon = player_render_info.cur_weapon;
        let cursor_dir = normalize(&player_render_info.cursor_pos); // TODO?
        let cursor_angle = Self::get_player_target_angle(&normalize(&cursor_dir));
        // now render the weapons
        let mut quad_scope = quad_scope_begin();
        quad_scope.set_state(&base_state);
        quad_scope.set_rotation(cursor_angle as f32); // TODO: (State.GetAttach()->m_Angle * pi * 2 + Angle)

        // normal weapons
        let texture = match current_weapon {
            WeaponType::Hammer => &weapon_texture.hammer.weapon.tex,
            WeaponType::Gun => &weapon_texture.gun.tex,
            WeaponType::Shotgun => &weapon_texture.shotgun.weapon.tex,
            WeaponType::Grenade => &weapon_texture.grenade.weapon.tex,
            WeaponType::Laser => &weapon_texture.laser.weapon.tex,
        };

        quad_scope.set_colors_from_single(1.0, 1.0, 1.0, 1.0); // TODO: <-- alpha

        let dir = vec2::new(cursor_dir.x as f32, cursor_dir.y as f32);

        let attack_time = player_render_info.recoil_ticks_passed.map(|recoil| {
            (intra_tick_time_to_ratio(game_info.intra_tick_time, ticks_in_a_second) + recoil as f64)
                / ticks_in_a_second as f64
        });

        let mut weapon_pos: vec2;
        if current_weapon == WeaponType::Hammer {
            let weapon_anim_state = TeeAnimationFrame::default(); // TODO:

            // static position for hammer
            weapon_pos = render_pos + vec2::new(weapon_anim_state.pos.x, weapon_anim_state.pos.y);
            let hammer_spec = WeaponHammerSpec::get();
            weapon_pos.y += hammer_spec.offset_y;

            if dir.x < 0.0 {
                weapon_pos.x -= hammer_spec.offset_x;
            }
            if is_sit {
                weapon_pos.y += 3.0 / 32.0;
            }

            // if active and attack is under way, bash stuffs
            if !inactive
            /* TODO: needed? ( || LastAttackTime < m_pClient->m_aTuning[g_Config.m_ClDummy].GetWeaponFireDelay(Player.m_Weapon))*/
            {
                let hammer_anim = hammer_swing_anim();
                // let hammer_spec = WeaponHammerSpec::get();
                // TODO: old client randomly uses times 5 here instead of some animation length let anim_time = hammer_spec.fire_delay as f64 / 1000.0;
                let attack_time =
                    attack_time.map(|attack_time| (attack_time * 5.0).clamp(0.0, 1.0));
                let mut frame = TeeAnimationFrame::default();

                AnimState::anim_frame_eval(
                    &hammer_anim,
                    &Duration::from_secs_f64(attack_time.unwrap_or(1.0)),
                    &mut frame,
                );
                if cursor_dir.x < 0.0 {
                    quad_scope.set_rotation(-PI / 2.0 - frame.rotation * PI * 2.0);
                } else {
                    quad_scope.set_rotation(-PI / 2.0 + frame.rotation * PI * 2.0);
                }
            } else {
                quad_scope.set_rotation(if cursor_dir.x < 0.0 { 100.0 } else { 500.0 });
            }

            let quad_offset = if cursor_dir.x >= 0.0 {
                self.weapon_quad_offsets.get(&WeaponType::Hammer).unwrap().0
            } else {
                self.weapon_quad_offsets.get(&WeaponType::Hammer).unwrap().1
            };

            self.quad_container.render_quad_container_as_sprite(
                quad_offset,
                weapon_pos.x,
                weapon_pos.y,
                1.0,
                1.0,
                quad_scope,
                texture.into(),
            );
        } else {
            // TODO: should be an animation
            let mut recoil = 0.0;
            let a = attack_time
                .map(|attack_time| attack_time * 10.0)
                .unwrap_or(1.0);
            if a < 1.0 {
                recoil = (a * PI as f64).sin();
            }
            let weapon_spec = match current_weapon {
                WeaponType::Hammer => panic!("this weapon should be handled earlier"),
                WeaponType::Gun => WeaponGunSpec::get(),
                WeaponType::Shotgun => WeaponShotgunSpec::get(),
                WeaponType::Grenade => WeaponGrenadeSpec::get(),
                WeaponType::Laser => WeaponLaserSpec::get(),
            };
            weapon_pos =
                render_pos + dir * weapon_spec.offset_x - dir * recoil as f32 * (10.0 / 32.0);
            weapon_pos.y += weapon_spec.offset_y;
            if is_sit {
                weapon_pos.y += 3.0 / 32.0;
            }

            let quad_offset = if cursor_dir.x >= 0.0 {
                self.weapon_quad_offsets.get(&current_weapon).unwrap().0
            } else {
                self.weapon_quad_offsets.get(&current_weapon).unwrap().1
            };

            self.quad_container.render_quad_container_as_sprite(
                quad_offset,
                weapon_pos.x,
                weapon_pos.y,
                1.0,
                1.0,
                quad_scope,
                texture.into(),
            );
        }

        // muzzle if weapon is firing
        if true {
            if current_weapon == WeaponType::Gun || current_weapon == WeaponType::Shotgun {
                // check if we're firing stuff
                let (weapon, spec) = if current_weapon == WeaponType::Gun {
                    (&weapon_texture.gun, WeaponGunSpec::get())
                } else {
                    (&weapon_texture.shotgun.weapon, WeaponShotgunSpec::get())
                };
                if !weapon.muzzles.is_empty() {
                    let mut alpha_muzzle = 0.0;
                    let muzzle_duration = 8.0 / 5.0; // TODO: move this into the weapon spec
                    let attack_time = attack_time
                        .map(|attack_time| attack_time * 10.0)
                        .unwrap_or(f64::MAX);
                    if attack_time < muzzle_duration {
                        alpha_muzzle = mix(&2.0, &0.0, 1.0_f64.min(0.0_f64.max(attack_time)));
                    }

                    if alpha_muzzle > 0.0 {
                        let mut pos_offset_y = -spec.muzzle_offset_y;
                        let quad_offset = if cursor_dir.x < 0.0 {
                            pos_offset_y = -pos_offset_y;
                            self.weapon_muzzle_quad_offsets
                                .get(&current_weapon)
                                .unwrap()
                                .1
                        } else {
                            self.weapon_muzzle_quad_offsets
                                .get(&current_weapon)
                                .unwrap()
                                .0
                        };

                        let muzzle_dir_y = vec2::new(-dir.y, dir.x);
                        let muzzle_pos =
                            weapon_pos + dir * spec.muzzle_offset_x + muzzle_dir_y * pos_offset_y;

                        let texture = weapon.muzzles.random_entry(&mut self.rng);

                        self.quad_container.render_quad_container_as_sprite(
                            quad_offset,
                            muzzle_pos.x,
                            muzzle_pos.y,
                            1.0,
                            1.0,
                            quad_scope,
                            texture.into(),
                        );
                    }
                }
            }
        }

        let dir_normalized = normalize(&dir);
        match current_weapon {
            WeaponType::Hammer => None,
            WeaponType::Gun => Some(TeeRenderHand {
                pos: weapon_pos - render_pos,
                dir: dir_normalized,
                rot_offset: -3.0 * PI / 4.0,
                offset_after_rot: vec2::new(-15.0, 4.0) / 32.0,
                scale: 1.0,
            }),
            WeaponType::Shotgun => Some(TeeRenderHand {
                pos: weapon_pos - render_pos,
                dir: dir_normalized,
                rot_offset: -PI / 2.0,
                offset_after_rot: vec2::new(-5.0, 4.0) / 32.0,
                scale: 1.0,
            }),
            WeaponType::Grenade => Some(TeeRenderHand {
                pos: weapon_pos - render_pos,
                dir: dir_normalized,
                rot_offset: -PI / 2.0,
                offset_after_rot: vec2::new(-4.0, 7.0) / 32.0,
                scale: 1.0,
            }),
            WeaponType::Laser => None,
        }
    }

    fn render_ninja_weapon(
        &mut self,
        ninjas: &mut NinjaContainer,
        particle_manager: &mut ParticleManager,
        player_render_info: &CharacterRenderInfo,
        game_info: &GameStateRenderInfo,
        ticks_in_a_second: GameTickType,
        cur_time: Duration,
        pos: vec2,
        is_sit: bool,
        base_state: State,
    ) -> Option<TeeRenderHand> {
        let mut weapon_pos = pos;
        let spec = WeaponNinjaSpec::get();
        weapon_pos.y += spec.offset_y;
        if is_sit {
            weapon_pos.y += 3.0;
        }

        let attack_time = player_render_info.recoil_ticks_passed.map(|recoil| {
            (intra_tick_time_to_ratio(game_info.intra_tick_time, ticks_in_a_second) + recoil as f64)
                / ticks_in_a_second as f64
        });

        let ninja_anim = ninja_swing_anim();
        // TODO: old client randomly uses times 2 here instead of some animation length let anim_time = ninja_spec.fire_delay as f64 / 1000.0;
        let attack_time = attack_time
            .map(|attack_time| (attack_time * 2.0).clamp(0.0, 1.0))
            .unwrap_or(1.0);
        let mut frame = TeeAnimationFrame::default();

        AnimState::anim_frame_eval(
            &ninja_anim,
            &Duration::from_secs_f64(attack_time),
            &mut frame,
        );

        let mut quad_scope = quad_scope_begin();
        quad_scope.set_state(&base_state);
        quad_scope.set_colors_from_single(1.0, 1.0, 1.0, 1.0); // TODO: <-- alpha

        let quad_offset = if player_render_info.cursor_pos.x < 0.0 {
            quad_scope.set_rotation(-PI / 2.0 - frame.rotation * PI * 2.0);
            weapon_pos.x -= spec.offset_x;
            Effects::new(particle_manager, cur_time).powerup_shine(
                &(weapon_pos + vec2::new(1.0, 0.0)),
                &vec2::new(1.0, 12.0 / 32.0),
            );
            self.ninja_quad_offsets.0
        } else {
            quad_scope.set_rotation(-PI / 2.0 + frame.rotation * PI * 2.0);
            Effects::new(particle_manager, cur_time).powerup_shine(
                &(weapon_pos - vec2::new(1.0, 0.0)),
                &vec2::new(1.0, 12.0 / 32.0),
            );
            self.ninja_quad_offsets.1
        };

        let ninja = ninjas.get_or_default(&"TODO".try_into().unwrap());
        self.quad_container.render_quad_container_as_sprite(
            quad_offset,
            weapon_pos.x,
            weapon_pos.y,
            1.0,
            1.0,
            quad_scope,
            (&ninja.weapon).into(),
        );

        // HADOKEN
        if attack_time <= 1.0 / 6.0 && !ninja.muzzles.is_empty() {
            let mut hadoken_angle = 0.0;
            let cursor = player_render_info.cursor_pos;
            let mut dir = vec2::new(cursor.x as f32, cursor.y as f32);
            if dir.x.abs() > 0.0001 || dir.y.abs() > 0.0001 {
                dir = normalize(&dir);
                hadoken_angle = angle(&dir);
            } else {
                dir = vec2::new(1.0, 0.0);
            }
            quad_scope.set_rotation(hadoken_angle);
            weapon_pos = pos;
            let offset_x = spec.muzzle_offset_x;
            weapon_pos -= dir * offset_x;

            self.quad_container.render_quad_container_as_sprite(
                self.ninja_muzzle_quad_offset,
                weapon_pos.x,
                weapon_pos.y,
                1.0,
                1.0,
                quad_scope,
                (ninja.muzzles.random_entry(&mut self.rng)).into(),
            );
        }

        None
    }

    pub fn render(&mut self, pipe: &mut PlayerRenderPipe) {
        let mut base_state = State::default();
        let center = pipe.camera.pos;
        self.canvas_mapping.map_canvas_for_ingame_items(
            &mut base_state,
            center.x,
            center.y,
            pipe.camera.zoom,
        );
        // first render the hooks
        // OLD: render everyone else's hook, then our own

        // intra tick
        // alpha other team
        // position (render pos)
        // hook (head, chain)
        // -> hand
        let ticks_in_a_second = pipe.game_info.ticks_per_second;

        let render_infos = pipe.render_infos;
        for (character_id, player_render_info) in render_infos.iter() {
            // dir to hook
            let pos = player_render_info.lerped_pos;

            let render_pos = pos;

            let vel = player_render_info.lerped_vel;
            let stationary = vel.x <= 1.0 / 32.0 && vel.x >= -1.0 / 32.0;
            let in_air = !pipe
                .collision
                .check_point(pos.x * 32.0, (pos.y + 0.5) * 32.0);
            let inactive = false; // TODO: m_pClient->m_aClients[ClientID].m_Afk || m_pClient->m_aClients[ClientID].m_Paused;
            let is_sit = inactive && !in_air && stationary;

            let vel_running = 5000.0 / 256.0;
            let input_dir = player_render_info.move_dir;
            let running = vel.x >= vel_running / 32.0 || vel.x <= -vel_running / 32.0;
            let want_other_dir =
                (input_dir == -1 && vel.x > 0.0) || (input_dir == 1 && vel.x < 0.0); // TODO: use input?

            let is_ninja = player_render_info.buffs.contains_key(&CharacterBuff::Ninja);
            let is_ghost = player_render_info.buffs.contains_key(&CharacterBuff::Ghost);
            let should_render_weapon = !is_ninja && !is_ghost;
            let should_render_hook = !is_ghost;

            // hook
            let hook_hand = should_render_hook
                .then(|| self.render_hook(pipe.hooks, pos, player_render_info, base_state))
                .flatten();

            let weapon_hand = if should_render_weapon {
                self.render_weapon(
                    pipe.weapons,
                    player_render_info,
                    render_pos,
                    ticks_in_a_second,
                    pipe.game_info,
                    base_state,
                    is_sit,
                    inactive,
                )
            } else if is_ninja {
                self.render_ninja_weapon(
                    pipe.ninjas,
                    pipe.particle_manager,
                    player_render_info,
                    pipe.game_info,
                    ticks_in_a_second,
                    *pipe.cur_time,
                    pos,
                    is_sit,
                    base_state,
                )
            } else {
                None
            };

            // in the end render the tees

            // OLD: render spectating players

            // OLD: render everyone else's tee, then our own
            // OLD: - hook cool
            // OLD: - player
            // OLD: - local player

            // for player and local player:

            // alpha other team
            // intra tick
            // weapon angle
            // direction and position
            // prepare render info
            // and determine animation
            // determine effects like stopping (bcs of direction change)
            // weapon animations
            // draw weapon => second hand
            // a shadow tee that shows unpredicted position
            // render tee
            // render state effects (frozen etc.)
            // render tee chatting <- state effect?
            // render afk state <- state effect?
            // render tee emote

            let state = base_state;

            let mut anim_state = AnimState::default();
            anim_state.set(&base_anim(), &Duration::from_millis(0));

            // evaluate animation
            let mut walk_time = pos.x.rem_euclid(100.0 / 32.0) / (100.0 / 32.0);
            let mut run_time = pos.x.rem_euclid(200.0 / 32.0) / (200.0 / 32.0);

            if in_air {
                anim_state.add(&inair_anim(), &Duration::from_millis(0), 1.0);
            } else if stationary {
                anim_state.add(&idle_anim(), &Duration::from_millis(0), 1.0);
            } else if !want_other_dir {
                if running {
                    anim_state.add(
                        &if vel.x < 0.0 {
                            run_left_anim()
                        } else {
                            run_right_anim()
                        },
                        &Duration::from_secs_f32(run_time),
                        1.0,
                    );
                } else {
                    anim_state.add(&walk_anim(), &Duration::from_secs_f32(walk_time), 1.0);
                }
            }

            let skin = if is_ninja {
                &pipe.ninjas.get_or_default(&"TODO".try_into().unwrap()).skin
            } else {
                let mut dummy = None;
                pipe.skins.get_or_default(
                    pipe.character_infos
                        .get(character_id)
                        .map(|char| &*char.skin)
                        .unwrap_or_else(|| {
                            dummy = Some("default".try_into().unwrap());
                            dummy.as_ref().unwrap()
                        }),
                )
            };
            let tee_render_info = TeeRenderInfo {
                render_skin: TeeRenderSkinTextures::Original(&skin.textures),
                color_body: ColorRGBA {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
                color_feet: ColorRGBA {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
                metrics: &skin.metrics,
                got_air_jump: player_render_info.has_air_jump,
                feet_flipped: false,
                size: RENDER_TEE_SIZE,
                eye_left: player_render_info.left_eye,
                eye_right: player_render_info.right_eye,
            };

            let dir = normalize(&player_render_info.cursor_pos);
            let dir = vec2::new(dir.x as f32, dir.y as f32);

            self.tee_renderer.render_tee(
                &anim_state,
                &tee_render_info,
                &TeeRenderHands {
                    left: hook_hand,
                    right: weapon_hand,
                },
                &dir,
                &render_pos,
                1.0,
                &state,
            );

            self.nameplate_renderer.render(&mut NameplateRenderPipe {
                cur_time: pipe.cur_time,
                name: "TODO:",
            });
            self.emoticon_renderer.render(&mut RenderEmoticonPipe {
                emoticon_container: pipe.emoticons,
                pos,
                emoticon: EmoticonType::HEARTS, // TODO: !!
            });
        }
    }
}
