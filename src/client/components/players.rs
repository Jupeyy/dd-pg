use std::{collections::HashMap, sync::Arc, time::Duration};

use base::system::System;
use base_io::io::IO;
use client_containers::{
    emoticons::{EmoticonType, EmoticonsContainer},
    hooks::HookContainer,
    skins::{SkinContainer, TeeSkinEye},
    weapons::WeaponContainer,
};
use client_render::{
    emoticons::render::{RenderEmoticon, RenderEmoticonPipe},
    nameplates::render::{NameplateRender, NameplateRenderPipe},
};
use client_render_base::{
    map::render_pipe::Camera,
    render::{
        animation::{AnimState, TeeAnimationFrame},
        canvas_mapping::map_canvas_for_ingame_items,
        default_anim::{
            base_anim, hammer_swing_anim, idle_anim, inair_anim, run_left_anim, run_right_anim,
            walk_anim,
        },
        tee::{RenderTee, TeeRenderHand, TeeRenderHands, TeeRenderInfo, TeeRenderSkinTextures},
    },
};
use config::config::ConfigEngine;
use graphics::{
    graphics::Graphics, handles::quad_container::QuadContainer, quad_container::Quad,
    streaming::quad_scope_begin,
};

use graphics_types::{
    commands::RenderSpriteInfo,
    rendering::{ColorRGBA, State},
};
use num_traits::FromPrimitive;
use shared::game::state_wasm_manager::GameStateWasmManager;
use shared_base::{
    game_types::intra_tick_time_to_ratio,
    network::messages::{WeaponType, NUM_WEAPONS},
};

use crate::{
    client::client::ClientData,
    render::weapons::{
        WeaponGrenadeSpec, WeaponGunSpec, WeaponHammerSpec, WeaponLaserSpec, WeaponShotgunSpec,
    },
};

use shared_game::{
    collision::collision::Collision,
    entities::character_core::character_core::HookState,
    state::state::GameStateInterface,
    weapons::definitions::weapon_def::{get_weapon_sprite_scale, get_weapon_visual_scale},
};

use math::math::{
    angle, distance, mix, normalize, random_int,
    vector::{dvec2, ubvec4, vec2},
    PI, PI_F64,
};

use super::render::get_sprite_scale_impl;

pub struct PlayerRenderPipe<'a> {
    pub graphics: &'a mut Graphics,
    pub sys: &'a System,
    pub runtime_thread_pool: &'a Arc<rayon::ThreadPool>,
    pub config: &'a mut ConfigEngine,
    pub game: &'a GameStateWasmManager,
    pub client_data: &'a ClientData,

    pub skins: &'a mut SkinContainer,
    pub hooks: &'a mut HookContainer,
    pub weapons: &'a mut WeaponContainer,
    pub emoticons: &'a mut EmoticonsContainer,

    pub collision: &'a Collision,
    pub io: &'a IO,
    pub camera: &'a Camera,
}

/**
 * The player component renders all hooks
 * all weapons, and all players
 */
pub struct Players {
    quad_container: QuadContainer,

    pub tee_renderer: RenderTee,
    nameplate_renderer: NameplateRender,
    emoticon_renderer: RenderEmoticon,

    hook_chain_quad_offset: usize,
    hook_head_quad_offset: usize,

    weapon_quad_offsets: HashMap<WeaponType, (usize, usize)>,
    weapon_muzzle_quad_offsets: HashMap<WeaponType, (usize, usize)>,
}

pub fn get_weapon_as_quad(weapon: &WeaponType) -> Quad {
    let size = get_weapon_visual_scale(weapon);
    let scale = get_weapon_sprite_scale(weapon);
    Quad::new().from_width_and_height_centered(size * scale.0, size * scale.1)
}

impl Players {
    pub fn new(graphics: &mut Graphics) -> Self {
        let tee_renderer = RenderTee::new(graphics);
        let nameplate_renderer = NameplateRender::new();
        let emoticon_renderer = RenderEmoticon::new(graphics);

        let mut quads: Vec<Quad> = Default::default();

        let mut weapon_quad_offsets: HashMap<WeaponType, (usize, usize)> = Default::default();

        weapon_quad_offsets.reserve(NUM_WEAPONS as usize);
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

        weapon_muzzle_quad_offsets.reserve(NUM_WEAPONS as usize);
        (0..NUM_WEAPONS).for_each(|wi| {
            let weapon = FromPrimitive::from_usize(wi).unwrap();
            let (scale_x, scale_y);
            if weapon == WeaponType::Gun || weapon == WeaponType::Shotgun {
                // TODO: hardcoded for now to get the same particle size as before
                (scale_x, scale_y) = get_sprite_scale_impl(96, 64);
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
            if weapon == WeaponType::Ninja {
                quads.push(quad.from_width_and_height_centered(160.0 * scale_x, 160.0 * scale_y));
            } else {
                quads.push(quad);
            };

            quad = quad.with_tex(&[
                vec2 { x: 0.0, y: 1.0 },
                vec2 { x: 1.0, y: 1.0 },
                vec2 { x: 1.0, y: 0.0 },
                vec2 { x: 0.0, y: 0.0 },
            ]);
            let offset_flipped = quads.len();
            if weapon == WeaponType::Ninja {
                quads.push(quad.from_width_and_height_centered(160.0 * scale_x, 160.0 * scale_y));
            } else {
                quads.push(quad);
            }

            weapon_muzzle_quad_offsets.insert(weapon, (offset_normal, offset_flipped));
        });

        let quad = Quad::new()
            .from_rect(-12.0, -8.0, 24.0, 16.0)
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
            tee_renderer,
            nameplate_renderer,
            emoticon_renderer,

            hook_chain_quad_offset,
            hook_head_quad_offset,

            weapon_quad_offsets,
            weapon_muzzle_quad_offsets,
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

    pub fn render(&mut self, pipe: &mut PlayerRenderPipe) {
        let mut base_state = State::default();
        let center = pipe.camera.pos;
        map_canvas_for_ingame_items(
            pipe.graphics,
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
        let ticks_in_a_second: u64 = pipe.game.game_tick_speed();

        let render_infos = pipe
            .game
            .collect_players_render_info(intra_tick_time_to_ratio(
                pipe.client_data.intra_tick_time,
                ticks_in_a_second,
            ));
        for player_render_info in render_infos {
            // dir to hook
            let pos = player_render_info.lerped_pos;
            let hook_pos = player_render_info.lerped_hook_pos;
            let hook_dir = normalize(&(pos - hook_pos));
            let cursor_dir = normalize(&player_render_info.cursor_pos); // TODO?
            let cursor_angle = Self::get_player_target_angle(&normalize(&cursor_dir));

            let render_pos = pos;

            let render_hook = player_render_info.hook_state >= HookState::RetractStart;
            // hook
            if render_hook {
                // current hook
                let cur_hook = pipe.hooks.get_or_default("TODO:");

                // render head
                let mut quad_scope = quad_scope_begin();
                quad_scope.set_state(&base_state);
                quad_scope.set_texture(&cur_hook.hook_head);
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
                );

                // render chain
                let mut hook_chain_render_info =
                    pipe.graphics.stream_handle.get_sprites_uniform_instance();
                let (sprites, used_count, instance) = hook_chain_render_info.get();
                let mut f = 24.0;
                let d = distance(&pos, &hook_pos);
                while f < d && *used_count < sprites.len() {
                    let p = hook_render_pos + hook_dir * f;
                    sprites[*used_count] = RenderSpriteInfo {
                        pos: p,
                        scale: 1.0,
                        rotation: angle(&hook_dir) + PI,
                    };
                    *used_count += 1;

                    f += 24.0;
                }
                let mut quad_scope = quad_scope_begin();
                quad_scope.set_state(&base_state);
                quad_scope.set_texture(&cur_hook.hook_chain);
                quad_scope.set_colors_from_single(1.0, 1.0, 1.0, 1.0);
                self.quad_container
                    .render_quad_container_as_sprite_multiple(
                        self.hook_chain_quad_offset,
                        instance,
                        *used_count,
                        quad_scope,
                    );
            }

            let current_weapon = player_render_info.cur_weapon;
            let weapon_texture = pipe.weapons.get_or_default("TODO:");

            let mut anim_state = AnimState::default();
            anim_state.set(&base_anim(), &Duration::from_millis(0));
            if current_weapon == WeaponType::Hammer {
                //TODO: anim_state.add(&hammer_swing_anim(), &Duration::from_millis(0), 1.0);
            }
            // now render the weapons
            let mut quad_scope = quad_scope_begin();
            quad_scope.set_state(&base_state);
            quad_scope.set_rotation(cursor_angle as f32); // TODO: (State.GetAttach()->m_Angle * pi * 2 + Angle)

            // normal weapons
            // TODO: int CurrentWeapon = clamp(Player.m_Weapon, 0, NUM_WEAPONS - 1);
            match current_weapon {
                WeaponType::Hammer => quad_scope.set_texture(&weapon_texture.hammer.tex),
                WeaponType::Gun => quad_scope.set_texture(&weapon_texture.gun.tex),
                WeaponType::Shotgun => quad_scope.set_texture(&weapon_texture.shotgun.tex),
                WeaponType::Grenade => quad_scope.set_texture(&weapon_texture.grenade.tex),
                WeaponType::Laser => quad_scope.set_texture(&weapon_texture.laser.tex),
                WeaponType::Ninja => todo!(), // TODO: remove
            }

            quad_scope.set_colors_from_single(1.0, 1.0, 1.0, 1.0); // TODO: <-- alpha

            let dir = vec2::new(cursor_dir.x as f32, cursor_dir.y as f32);
            //let WeaponPosition: vec2;

            let vel = player_render_info.lerped_vel;
            let stationary = vel.x <= 1.0 && vel.x >= -1.0;
            let in_air = !pipe.collision.check_point(pos.x, pos.y + 16.0);
            let running = pos.x >= 5000.0 || pos.x <= -5000.0;
            let input_dir = player_render_info.move_dir;
            let want_other_dir =
                (input_dir == -1 && vel.x > 0.0) || (input_dir == 1 && vel.x < 0.0); // TODO: use input?
            let inactive = false; // TODO: m_pClient->m_aClients[ClientID].m_Afk || m_pClient->m_aClients[ClientID].m_Paused;
            let is_sit = inactive && !in_air && stationary;

            let cur_tick = pipe.game.cur_monotonic_tick();

            let attack_time =
                (intra_tick_time_to_ratio(pipe.client_data.intra_tick_time, ticks_in_a_second)
                    + cur_tick as f64
                    - player_render_info.recoil_start_tick as f64)
                    / ticks_in_a_second as f64;

            let attack_ticks_passed = attack_time * ticks_in_a_second as f64;

            let mut weapon_pos: vec2;
            if current_weapon == WeaponType::Hammer {
                let weapon_anim_state = TeeAnimationFrame::default(); // TODO:
                                                                      // static position for hammer
                weapon_pos =
                    render_pos + vec2::new(weapon_anim_state.pos.x, weapon_anim_state.pos.y);
                let hammer_spec = WeaponHammerSpec::get();
                weapon_pos.y += hammer_spec.offset_y;

                if dir.x < 0.0 {
                    weapon_pos.x -= hammer_spec.offset_x;
                }
                if is_sit {
                    weapon_pos.y += 3.0;
                }

                // if active and attack is under way, bash stuffs
                if !inactive
                /* TODO: needed? ( || LastAttackTime < m_pClient->m_aTuning[g_Config.m_ClDummy].GetWeaponFireDelay(Player.m_Weapon))*/
                {
                    let hammer_anim = hammer_swing_anim();
                    // let hammer_spec = WeaponHammerSpec::get();
                    // TODO: old client randomly uses times 5 here instead of some animation length let anim_time = hammer_spec.fire_delay as f64 / 1000.0;
                    let attack_time = (attack_time * 5.0).clamp(0.0, 1.0);
                    let mut frame = TeeAnimationFrame::default();

                    AnimState::anim_frame_eval(
                        &hammer_anim,
                        &Duration::from_secs_f64(attack_time),
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
                );
            }
            /*
            else if(Player.m_Weapon == WEAPON_NINJA)
            {
                WeaponPosition = Position;
                WeaponPosition.y += g_pData->m_Weapons.m_aId[CurrentWeapon].m_Offsety;
                if(IsSit)
                    WeaponPosition.y += 3.0f;

                if(Direction.x < 0)
                {
                    Graphics()->QuadsSetRotation(-pi / 2 - State.GetAttach()->m_Angle * pi * 2);
                    WeaponPosition.x -= g_pData->m_Weapons.m_aId[CurrentWeapon].m_Offsetx;
                    m_pClient->m_Effects.PowerupShine(WeaponPosition + vec2(32, 0), vec2(32, 12));
                }
                else
                {
                    Graphics()->QuadsSetRotation(-pi / 2 + State.GetAttach()->m_Angle * pi * 2);
                    m_pClient->m_Effects.PowerupShine(WeaponPosition - vec2(32, 0), vec2(32, 12));
                }
                Graphics()->RenderQuadContainerAsSprite(m_WeaponEmoteQuadContainerIndex, QuadOffset, WeaponPosition.x, WeaponPosition.y);

                // HADOKEN
                if(AttackTime <= 1 / 6.f && g_pData->m_Weapons.m_aId[CurrentWeapon].m_NumSpriteMuzzles)
                {
                    int IteX = rand() % g_pData->m_Weapons.m_aId[CurrentWeapon].m_NumSpriteMuzzles;
                    static int s_LastIteX = IteX;
                    if(Client()->State() == IClient::STATE_DEMOPLAYBACK)
                    {
                        const IDemoPlayer::CInfo *pInfo = DemoPlayer()->BaseInfo();
                        if(pInfo->m_Paused)
                            IteX = s_LastIteX;
                        else
                            s_LastIteX = IteX;
                    }
                    else
                    {
                        if(m_pClient->m_Snap.m_pGameInfoObj && m_pClient->m_Snap.m_pGameInfoObj->m_GameStateFlags & GAMESTATEFLAG_PAUSED)
                            IteX = s_LastIteX;
                        else
                            s_LastIteX = IteX;
                    }
                    if(g_pData->m_Weapons.m_aId[CurrentWeapon].m_aSpriteMuzzles[IteX])
                    {
                        if(PredictLocalWeapons)
                            dir = vec2(pPlayerChar->m_X, pPlayerChar->m_Y) - vec2(pPrevChar->m_X, pPrevChar->m_Y);
                        else
                            dir = vec2(m_pClient->m_Snap.m_aCharacters[ClientID].m_Cur.m_X, m_pClient->m_Snap.m_aCharacters[ClientID].m_Cur.m_Y) - vec2(m_pClient->m_Snap.m_aCharacters[ClientID].m_Prev.m_X, m_pClient->m_Snap.m_aCharacters[ClientID].m_Prev.m_Y);
                        float HadOkenAngle = 0;
                        if(absolute(dir.x) > 0.0001f || absolute(dir.y) > 0.0001f)
                        {
                            dir = normalize(dir);
                            HadOkenAngle = angle(dir);
                        }
                        else
                        {
                            dir = vec2(1, 0);
                        }
                        Graphics()->QuadsSetRotation(HadOkenAngle);
                        QuadOffset = IteX * 2;
                        vec2 DirY(-dir.y, dir.x);
                        WeaponPosition = Position;
                        float OffsetX = g_pData->m_Weapons.m_aId[CurrentWeapon].m_Muzzleoffsetx;
                        WeaponPosition -= dir * OffsetX;
                        Graphics()->TextureSet(GameClient()->m_GameSkin.m_aaSpriteWeaponsMuzzles[CurrentWeapon][IteX]);
                        Graphics()->RenderQuadContainerAsSprite(m_aWeaponSpriteMuzzleQuadContainerIndex[CurrentWeapon], QuadOffset, WeaponPosition.x, WeaponPosition.y);
                    }
                }
            }
            */
            else {
                // TODO: should be an animation
                let mut recoil = 0.0;
                let a = attack_ticks_passed / 5.0;
                if a < 1.0 {
                    recoil = (a * PI as f64).sin();
                }
                let weapon_spec = match current_weapon {
                    WeaponType::Hammer => panic!("this weapon should be handled earlier"),
                    WeaponType::Gun => WeaponGunSpec::get(),
                    WeaponType::Shotgun => WeaponShotgunSpec::get(),
                    WeaponType::Grenade => WeaponGrenadeSpec::get(),
                    WeaponType::Laser => WeaponLaserSpec::get(),
                    WeaponType::Ninja => panic!("this weapon should be handled earlier"),
                };
                weapon_pos = render_pos + dir * weapon_spec.offset_x - dir * recoil as f32 * 10.0;
                weapon_pos.y += weapon_spec.offset_y;
                if is_sit {
                    weapon_pos.y += 3.0;
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
                );
            }

            // muzzle if weapon is firing
            if true {
                if current_weapon == WeaponType::Gun || current_weapon == WeaponType::Shotgun {
                    // check if we're firing stuff

                    let (weapon, spec) = if current_weapon == WeaponType::Gun {
                        (&weapon_texture.gun, WeaponGunSpec::get())
                    } else {
                        (&weapon_texture.shotgun, WeaponShotgunSpec::get())
                    };
                    if !weapon.muzzles.is_empty() {
                        let mut alpha_muzzle = 0.0;
                        let muzzle_duration = 5.0; // TODO: move this into the weapon spec
                        if attack_ticks_passed < muzzle_duration + 3.0 {
                            let t = attack_ticks_passed / muzzle_duration;
                            alpha_muzzle = mix(&2.0, &0.0, 1.0_f64.min(0.0_f64.max(t)));
                        }

                        let muzzle_index = random_int() as usize % weapon.muzzles.len();

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
                            let muzzle_pos = weapon_pos
                                + dir * spec.muzzle_offset_x
                                + muzzle_dir_y * pos_offset_y;

                            quad_scope.set_texture(&weapon.muzzles[muzzle_index]);

                            self.quad_container.render_quad_container_as_sprite(
                                quad_offset,
                                muzzle_pos.x,
                                muzzle_pos.y,
                                1.0,
                                1.0,
                                quad_scope,
                            );
                        }
                    }
                }
            }

            /*
            Graphics()->SetColor(1.0f, 1.0f, 1.0f, 1.0f);
            Graphics()->QuadsSetRotation(0); */

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
            let mut walk_time = pos.x.rem_euclid(100.0) / 100.0;
            let mut run_time = pos.x.rem_euclid(200.0) / 200.0;

            // Don't do a moon walk outside the left border
            if walk_time < 0.0 {
                walk_time += 1.0;
            }
            if run_time < 0.0 {
                run_time += 1.0;
            }

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

            let skin = pipe.skins.get_or_default(&player_render_info.skin_name);
            let tee_render_info = TeeRenderInfo {
                render_skin: TeeRenderSkinTextures::Colorable(&skin.grey_scaled_textures),
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
                got_air_jump: false,
                feet_flipped: false,
                size: 64.0,
            };

            let dir = normalize(&player_render_info.cursor_pos);
            let dir = vec2::new(dir.x as f32, dir.y as f32);

            self.tee_renderer.render_tee(
                &anim_state,
                &tee_render_info,
                TeeSkinEye::Normal,
                &TeeRenderHands {
                    left: if render_hook {
                        Some(TeeRenderHand {
                            pos: vec2::new(0.0, 0.0),
                            dir: hook_dir,
                            scale: 1.0,
                        })
                    } else {
                        None
                    },
                    right: Some(TeeRenderHand {
                        pos: vec2::new(0.0, 0.0),
                        dir,
                        scale: 1.0,
                    }),
                },
                &dir,
                &render_pos,
                1.0,
                &state,
            );

            self.nameplate_renderer.render(&mut NameplateRenderPipe {
                sys: pipe.sys,
                config: pipe.config,
                graphics: pipe.graphics,
                name: "TODO:",
            });
            self.emoticon_renderer.render(&mut RenderEmoticonPipe {
                graphics: pipe.graphics,
                emoticon_container: pipe.emoticons,
                runtime_thread_pool: pipe.runtime_thread_pool,
                io: pipe.io,
                pos,
                emoticon: EmoticonType::HEARTS, // TODO: !!
            });
        }
    }
}
