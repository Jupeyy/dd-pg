use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use base::system::System;
use base_fs::{filesys::FileSystem, io_batcher::TokIOBatcher};
use config::config::Config;
use graphics_render_traits::{GraphicsRenderGeometry, GraphicsRenderQuadContainer};
use graphics_traits::GraphicsSizeQuery;
use graphics_types::{
    command_buffer::SRenderSpriteInfo,
    rendering::{ColorRGBA, State},
    types::QuadContainerIndex,
};
use num_traits::FromPrimitive;
use shared::game::state_wasm_manager::GameStateWasmManager;
use shared_base::game_types::intra_tick_time_to_ratio;

use crate::{
    client::{
        client::ClientData, component::ComponentDestroyPipe, render_pipe::Camera,
        render_tools::RenderTools,
    },
    containers::{hooks::HookContainer, skins::SkinContainer, weapons::WeaponContainer},
    render::{
        animation::{AnimState, TeeAnimationFrame},
        default_anim::{
            base_anim, idle_anim, inair_anim, run_left_anim, run_right_anim, walk_anim,
        },
        tee::{RenderTee, TeeEyeEmote, TeeRenderInfo, TeeRenderSkinTextures},
        weapons::{WeaponGunSpec, WeaponHammerSpec},
    },
};

use shared_game::{
    collision::collision::Collision,
    entities::character_core::character_core::HookState,
    player::player::PlayerRenderInfo,
    state::state::GameStateInterface,
    weapons::definitions::weapon_def::{
        get_weapon_sprite_scale, get_weapon_visual_scale, WeaponType,
    },
};

use math::math::{
    angle, distance, normalize,
    vector::{dvec2, ubvec4, vec2},
    PId, PI,
};

use graphics::graphics::{
    Graphics, GraphicsQuadContainerInterface, QuadContainerBuilder, QuadContainerRenderCount, SQuad,
};

pub struct PlayerRenderPipe<'a> {
    pub graphics: &'a mut Graphics,
    pub sys: &'a System,
    pub runtime_thread_pool: &'a Arc<rayon::ThreadPool>,
    pub config: &'a mut Config,
    pub game: &'a GameStateWasmManager,
    pub client_data: &'a ClientData,
    pub skins: &'a mut SkinContainer,
    pub hooks: &'a mut HookContainer,
    pub weapons: &'a mut WeaponContainer,
    pub collision: &'a Collision,
    pub fs: &'a Arc<FileSystem>,
    pub io_batcher: &'a Arc<Mutex<TokIOBatcher>>,
    pub camera: &'a Camera,
}

/**
 * The player component renders all hooks
 * all weapons, and all players
 */
pub struct Players {
    quad_container_index: QuadContainerIndex,

    tee_renderer: RenderTee,

    hook_chain_quad_offset: usize,
    hook_head_quad_offset: usize,

    weapon_quad_offsets: HashMap<WeaponType, (usize, usize)>,

    render_info_helper: Vec<PlayerRenderInfo>,
}

pub fn get_weapon_as_quad(weapon: &WeaponType) -> SQuad {
    let size = get_weapon_visual_scale(weapon);
    let scale = get_weapon_sprite_scale(weapon);
    *SQuad::new().from_width_and_height_centered(size * scale.0, size * scale.1)
}

impl Players {
    pub fn new(graphics: &mut Graphics) -> Self {
        let tee_renderer = RenderTee::new(graphics);

        let quad_container_index =
            graphics.create_quad_container(&QuadContainerBuilder::new(false));

        let mut weapon_quad_offsets: HashMap<WeaponType, (usize, usize)> = Default::default();

        weapon_quad_offsets.reserve(WeaponType::NumWeapons as usize);
        (0..WeaponType::NumWeapons as usize)
            .enumerate()
            .for_each(|(index, wi)| {
                let mut quad = *get_weapon_as_quad(&FromPrimitive::from_usize(wi).unwrap())
                    .with_color(&ubvec4::new(255, 255, 255, 255));

                quad.with_uv_from_points(&vec2::new(0.0, 0.0), &vec2::new(1.0, 1.0));
                let offset_normal =
                    graphics.quad_container_add_quads(&quad_container_index, &[quad]);
                quad.with_uv_from_points(&vec2::new(0.0, 1.0), &vec2::new(1.0, 0.0));
                let offset_flipped =
                    graphics.quad_container_add_quads(&quad_container_index, &[quad]);
                weapon_quad_offsets.insert(
                    WeaponType::from_usize(index).unwrap(),
                    (offset_normal, offset_flipped),
                );
            });

        let quad = *SQuad::new()
            .from_rect(-12.0, -8.0, 24.0, 16.0)
            .with_color(&ubvec4::new(255, 255, 255, 255))
            .with_uv_from_points(&vec2::new(0.0, 0.0), &vec2::new(1.0, 1.0));

        let hook_chain_quad_offset =
            graphics.quad_container_add_quads(&quad_container_index, &[quad]);
        let hook_head_quad_offset = hook_chain_quad_offset;

        graphics.quad_container_upload(&quad_container_index);
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
            quad_container_index,
            tee_renderer,

            hook_chain_quad_offset,
            hook_head_quad_offset,
            weapon_quad_offsets,

            render_info_helper: Vec::with_capacity(64),
        }
    }

    pub fn destroy(self, pipe: &mut ComponentDestroyPipe) {
        pipe.graphics
            .delete_quad_container(self.quad_container_index);

        self.tee_renderer.destroy(pipe.graphics);
    }

    pub fn map_canvas_for_players(
        graphics: &Graphics,
        state: &mut State,
        center_x: f32,
        center_y: f32,
        zoom: f32,
    ) {
        let points: [f32; 4] = RenderTools::map_canvas_to_world(
            center_x,
            center_y,
            0.0,
            0.0,
            1.0,
            0.0,
            0.0,
            graphics.canvas_aspect(),
            zoom,
        );
        state.map_canvas(points[0], points[1], points[2], points[3]);
    }

    fn get_player_target_angle(cursor_vec: &dvec2) -> f64 {
        if cursor_vec.x == 0.0 && cursor_vec.y == 0.0 {
            return 0.0;
        } else if cursor_vec.x == 0.0 {
            return if cursor_vec.y < 0.0 {
                -PId / 2.0
            } else {
                PId / 2.0
            };
        }
        let tmp_angle = (cursor_vec.y / cursor_vec.x).atan();
        if cursor_vec.x < 0.0 {
            tmp_angle + PId
        } else {
            tmp_angle
        }
    }

    pub fn render(&mut self, pipe: &mut PlayerRenderPipe) {
        let mut base_state = State::default();
        let center = pipe.camera.pos;
        Self::map_canvas_for_players(
            &pipe.graphics,
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
        pipe.game.collect_players_render_info(
            intra_tick_time_to_ratio(pipe.client_data.intra_tick_time),
            &mut self.render_info_helper,
        );
        for player_render_info in self.render_info_helper.drain(..) {
            // dir to hook
            let pos = player_render_info.lerped_pos;
            let hook_pos = player_render_info.lerped_hook_pos;
            let dir = normalize(&(pos - hook_pos));
            let cursor_dir = normalize(&player_render_info.cursor_pos); // TODO?
            let cursor_angle = Self::get_player_target_angle(&normalize(&cursor_dir));

            let pos_to_camera = pos - center;

            // hook
            if player_render_info.hook_state >= HookState::RetractStart {
                // current hook
                let cur_hook =
                    pipe.hooks
                        .get_or_default("TODO:", pipe.graphics, pipe.fs, pipe.io_batcher);

                // render head
                let mut quad_scope = pipe.graphics.backend_handle.quad_scope_begin();
                quad_scope.set_state(&base_state);
                quad_scope.set_texture(&cur_hook.hook_head);
                quad_scope.set_rotation(angle(&dir) + PI);
                quad_scope.set_colors_from_single(1.0, 1.0, 1.0, 1.0); //<-- alpha

                let hook_render_pos = hook_pos - center;
                pipe.graphics
                    .quad_container_handle
                    .render_quad_container_as_sprite(
                        &self.quad_container_index,
                        self.hook_head_quad_offset,
                        hook_render_pos.x,
                        hook_render_pos.y,
                        1.0,
                        1.0,
                        quad_scope,
                    );

                // render chain
                let mut hook_chain_render_info = pipe.graphics.sprite_render_info_pool.new();
                let mut f = 24.0;
                let d = distance(&pos, &hook_pos);
                while f < d && hook_chain_render_info.len() < 1024 {
                    let p = hook_render_pos + dir * f;
                    hook_chain_render_info.push(SRenderSpriteInfo {
                        pos: p,
                        scale: 1.0,
                        rotation: angle(&dir) + PI,
                    });

                    f += 24.0;
                }
                let mut quad_scope = pipe.graphics.backend_handle.quad_scope_begin();
                quad_scope.set_state(&base_state);
                quad_scope.set_texture(&cur_hook.hook_chain);
                quad_scope.set_colors_from_single(1.0, 1.0, 1.0, 1.0);
                pipe.graphics
                    .quad_container_handle
                    .render_quad_container_as_sprite_multiple(
                        &self.quad_container_index,
                        self.hook_chain_quad_offset,
                        &QuadContainerRenderCount::Count(hook_chain_render_info.len()),
                        hook_chain_render_info,
                        quad_scope,
                    );
            }

            let current_weapon = player_render_info.cur_weapon;
            let weapon_texture = pipe.weapons.get_or_default(
                current_weapon.to_string(),
                pipe.graphics,
                pipe.fs,
                pipe.io_batcher,
            );

            let mut anim_state = AnimState::default();
            anim_state.set(&base_anim(), &Duration::from_millis(0));
            if current_weapon == WeaponType::Hammer {
                //TODO: anim_state.add(&hammer_swing_anim(), &Duration::from_millis(0), 1.0);
            }
            // now render the weapons
            let mut quad_scope = pipe.graphics.backend_handle.quad_scope_begin();
            quad_scope.set_state(&base_state);
            quad_scope.set_rotation(cursor_angle as f32); // TODO: (State.GetAttach()->m_Angle * pi * 2 + Angle)

            // normal weapons
            // TODO: int CurrentWeapon = clamp(Player.m_Weapon, 0, NUM_WEAPONS - 1);
            quad_scope.set_texture(&weapon_texture.gun.tex);

            let quad_offset = self.weapon_quad_offsets.get(&current_weapon).unwrap();

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
            let ticks_in_a_second = pipe.game.game_tick_speed();

            let attack_time = (intra_tick_time_to_ratio(pipe.client_data.intra_tick_time)
                + cur_tick as f64
                - player_render_info.recoil_start_tick as f64)
                / ticks_in_a_second as f64;

            let attack_ticks_passed = attack_time * ticks_in_a_second as f64;

            if current_weapon == WeaponType::Hammer {
                let weapon_anim_state = TeeAnimationFrame::default(); // TODO:
                                                                      // static position for hammer
                let mut weapon_pos =
                    pos_to_camera + vec2::new(weapon_anim_state.pos.x, weapon_anim_state.pos.y);
                let hammer_spec = WeaponHammerSpec::get();
                weapon_pos.y += hammer_spec.offset_y;

                if dir.x < 0.0 {
                    weapon_pos.x -= hammer_spec.offset_x;
                }
                if is_sit {
                    weapon_pos.y += 3.0;
                }

                // if active and attack is under way, bash stuffs
                /* TODO: if(!Inactive || LastAttackTime < m_pClient->m_aTuning[g_Config.m_ClDummy].GetWeaponFireDelay(Player.m_Weapon))
                {
                    if(dir.x < 0) {
                        quad_scope.set_rotation(-pi / 2 - State.GetAttach()->m_Angle * pi * 2);
                    }
                    else {
                        quad_scope.set_rotation(-pi / 2 + State.GetAttach()->m_Angle * pi * 2);
                    }
                }
                else {
                    quad_scope.set_rotation(dir.x < 0 ? 100.0 : 500.0);
                }*/

                let quad_offset = if cursor_dir.x < 0.0 {
                    self.weapon_quad_offsets.get(&WeaponType::Hammer).unwrap().0
                } else {
                    self.weapon_quad_offsets.get(&WeaponType::Hammer).unwrap().1
                };

                pipe.graphics
                    .quad_container_handle
                    .render_quad_container_as_sprite(
                        &self.quad_container_index,
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
                let gun_spec = WeaponGunSpec::get();
                let mut weapon_pos =
                    pos_to_camera + dir * gun_spec.offset_x - dir * recoil as f32 * 10.0;
                weapon_pos.y += gun_spec.offset_y;
                if is_sit {
                    weapon_pos.y += 3.0;
                }
                if current_weapon == WeaponType::Gun {
                    weapon_pos.y -= 8.0;
                }

                let quad_offset = if cursor_dir.x >= 0.0 {
                    self.weapon_quad_offsets.get(&WeaponType::Gun).unwrap().0
                } else {
                    self.weapon_quad_offsets.get(&WeaponType::Gun).unwrap().1
                };

                pipe.graphics
                    .quad_container_handle
                    .render_quad_container_as_sprite(
                        &self.quad_container_index,
                        quad_offset,
                        weapon_pos.x,
                        weapon_pos.y,
                        1.0,
                        1.0,
                        quad_scope,
                    );
            }

            // swizzle if weapon is firing
            if true {
                if current_weapon == WeaponType::Gun || current_weapon == WeaponType::Shotgun {
                    // check if we're firing stuff
                    /*if(g_pData->m_Weapons.m_aId[CurrentWeapon].m_NumSpriteMuzzles) // prev.attackticks)
                    {
                        float AlphaMuzzle = 0.0f;
                        if(AttackTicksPassed < g_pData->m_Weapons.m_aId[CurrentWeapon].m_Muzzleduration + 3)
                        {
                            float t = AttackTicksPassed / g_pData->m_Weapons.m_aId[CurrentWeapon].m_Muzzleduration;
                            AlphaMuzzle = mix(2.0f, 0.0f, minimum(1.0f, maximum(0.0f, t)));
                        }

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
                        if(AlphaMuzzle > 0.0f && g_pData->m_Weapons.m_aId[CurrentWeapon].m_aSpriteMuzzles[IteX])
                        {
                            float OffsetY = -g_pData->m_Weapons.m_aId[CurrentWeapon].m_Muzzleoffsety;
                            QuadOffset = IteX * 2 + (Direction.x < 0 ? 1 : 0);
                            if(Direction.x < 0)
                                OffsetY = -OffsetY;

                            vec2 DirY(-dir.y, dir.x);
                            vec2 MuzzlePos = WeaponPosition + dir * g_pData->m_Weapons.m_aId[CurrentWeapon].m_Muzzleoffsetx + DirY * OffsetY;
                            Graphics()->TextureSet(GameClient()->m_GameSkin.m_aaSpriteWeaponsMuzzles[CurrentWeapon][IteX]);
                            Graphics()->RenderQuadContainerAsSprite(m_aWeaponSpriteMuzzleQuadContainerIndex[CurrentWeapon], QuadOffset, MuzzlePos.x, MuzzlePos.y);
                        }
                    }*/
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

            let tee_render_info = TeeRenderInfo {
                render_skin: TeeRenderSkinTextures::Original(pipe.skins.get_or_default(
                    "TODO:",
                    pipe.graphics,
                    pipe.fs,
                    pipe.io_batcher,
                )),
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
                got_air_jump: false,
                feet_flipped: false,
                size: 64.0,
            };

            let dir = normalize(&player_render_info.cursor_pos);
            let dir = vec2::new(dir.x as f32, dir.y as f32);

            self.tee_renderer.render_tee(
                pipe.graphics,
                &anim_state,
                &tee_render_info,
                TeeEyeEmote::Normal,
                &dir,
                &pos_to_camera,
                1.0,
                &state,
            );
        }
    }
}
