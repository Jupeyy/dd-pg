use std::{borrow::Borrow, time::Duration};

use client_containers::{
    emoticons::EmoticonsContainer,
    freezes::FreezeContainer,
    hooks::HookContainer,
    ninja::NinjaContainer,
    skins::{Skin, SkinContainer},
    weapons::WeaponContainer,
};
use client_render::{
    emoticons::render::{RenderEmoticon, RenderEmoticonPipe},
    nameplates::render::{NameplateRender, NameplateRenderPipe},
};
use client_render_base::{
    map::render_pipe::{Camera, GameTimeInfo},
    render::{
        animation::AnimState,
        canvas_mapping::CanvasMappingIngame,
        default_anim::{
            base_anim, idle_anim, inair_anim, run_left_anim, run_right_anim, walk_anim,
        },
        particle_manager::ParticleManager,
        tee::{RenderTee, RenderTeeHandMath, TeeRenderHands, TeeRenderInfo, TeeRenderSkinColor},
        toolkit::ToolkitRender,
    },
};
use graphics::graphics::graphics::Graphics;

use graphics_types::rendering::State;
use pool::datatypes::PoolLinkedHashMap;

use shared_game::collision::collision::Collision;

use game_interface::types::{
    character_info::NetworkSkinInfo,
    game::GameEntityId,
    render::character::{CharacterBuff, CharacterDebuff, CharacterInfo, CharacterRenderInfo},
    resource_key::NetworkResourceKey,
};
use math::math::{normalize, vector::vec2};
use ui_base::ui::UiCreator;

pub struct PlayerRenderPipe<'a> {
    pub cur_time: &'a Duration,
    pub game_time_info: &'a GameTimeInfo,
    pub render_infos: &'a PoolLinkedHashMap<GameEntityId, CharacterRenderInfo>,
    pub character_infos: &'a PoolLinkedHashMap<GameEntityId, CharacterInfo>,

    pub skins: &'a mut SkinContainer,
    pub ninjas: &'a mut NinjaContainer,
    pub freezes: &'a mut FreezeContainer,
    pub hooks: &'a mut HookContainer,
    pub weapons: &'a mut WeaponContainer,
    pub emoticons: &'a mut EmoticonsContainer,

    pub particle_manager: &'a mut ParticleManager,

    pub collision: &'a Collision,
    pub camera: &'a Camera,

    pub own_character: Option<&'a GameEntityId>,
}

/// The player component renders all hooks
/// all weapons, and all players
pub struct Players {
    canvas_mapping: CanvasMappingIngame,

    pub tee_renderer: RenderTee,
    nameplate_renderer: NameplateRender,
    emoticon_renderer: RenderEmoticon,
    pub toolkit_renderer: ToolkitRender,
}

impl Players {
    pub fn new(graphics: &Graphics, creator: &UiCreator) -> Self {
        let tee_renderer = RenderTee::new(graphics);
        let nameplate_renderer = NameplateRender::new(graphics, creator);
        let emoticon_renderer = RenderEmoticon::new(graphics);
        let toolkit_renderer = ToolkitRender::new(graphics);

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
            canvas_mapping: CanvasMappingIngame::new(graphics),

            tee_renderer,
            nameplate_renderer,
            emoticon_renderer,
            toolkit_renderer,
        }
    }

    fn base_state(&self, camera: &Camera) -> State {
        let mut base_state = State::default();
        let center = camera.pos;
        self.canvas_mapping.map_canvas_for_ingame_items(
            &mut base_state,
            center.x,
            center.y,
            camera.zoom,
        );
        base_state
    }

    fn render_info_iter<'a>(
        render_infos: &'a PoolLinkedHashMap<GameEntityId, CharacterRenderInfo>,
        own_character: &'a Option<&'a GameEntityId>,
    ) -> impl Iterator<Item = (&'a GameEntityId, &'a CharacterRenderInfo)> {
        render_infos
            .iter()
            .filter(move |(id, _)| !own_character.is_some_and(|own_id| own_id.eq(id)))
            .chain(own_character.and_then(|id| render_infos.get_key_value(id)))
    }

    pub fn render(&mut self, pipe: &mut PlayerRenderPipe) {
        // first render the hooks
        // OLD: render everyone else's hook, then our own

        // intra tick
        // alpha other team
        // position (render pos)
        // hook (head, chain)
        // -> hand
        let ticks_in_a_second = pipe.game_time_info.ticks_per_second;
        let PlayerRenderPipe {
            cur_time,
            game_time_info,
            render_infos,
            character_infos,
            skins,
            ninjas,
            freezes,
            hooks,
            weapons,
            emoticons,
            particle_manager,
            collision,
            own_character,
            camera,
        } = pipe;

        let state = self.base_state(camera);

        let alpha = 1.0;

        const RENDER_TEE_SIZE: f32 = 2.0;

        fn skin_colors(
            character_info: Option<&CharacterInfo>,
        ) -> (TeeRenderSkinColor, TeeRenderSkinColor) {
            if let Some(NetworkSkinInfo::Custom {
                body_color,
                feet_color,
            }) = character_info.map(|character_info| character_info.skin_info)
            {
                (body_color.into(), feet_color.into())
            } else {
                (TeeRenderSkinColor::Original, TeeRenderSkinColor::Original)
            }
        }

        fn skin<'a>(
            character_info: Option<&'a CharacterInfo>,
            ninja_skin: Option<Option<&NetworkResourceKey<24>>>,
            freeze_skin: Option<Option<&NetworkResourceKey<24>>>,
            freezes: &'a mut FreezeContainer,
            ninjas: &'a mut NinjaContainer,
            skins: &'a mut SkinContainer,
        ) -> &'a Skin {
            if let Some(freeze_skin) = freeze_skin {
                &freezes.get_or_default_opt(freeze_skin).skin
            } else if let Some(ninja_skin) = ninja_skin {
                &ninjas.get_or_default_opt(ninja_skin).skin
            } else {
                skins.get_or_default_opt(character_info.map(|char| &char.info.skin))
            }
        }

        // first render all hooks
        for (character_id, player_render_info) in
            Self::render_info_iter(render_infos, own_character)
        {
            let pos = player_render_info.lerped_pos;
            let is_freeze = player_render_info
                .debuffs
                .contains_key(&CharacterDebuff::Freeze);
            let is_ninja = player_render_info.buffs.contains_key(&CharacterBuff::Ninja);
            let is_ghost = player_render_info.buffs.contains_key(&CharacterBuff::Ghost);
            let should_render_hook = !is_ghost;

            let character_info = character_infos.get(character_id);
            let freeze_skin = is_freeze.then(|| character_info.map(|char| &char.info.freeze));
            let ninja_skin = is_ninja.then(|| character_info.map(|char| &char.info.ninja));

            let (color_body, _) = skin_colors(character_info);

            // hook
            let hook_hand = should_render_hook
                .then(|| {
                    self.toolkit_renderer.render_hook_for_player(
                        hooks,
                        character_info.map(|char| char.info.hook.borrow()),
                        pos,
                        player_render_info,
                        state,
                    )
                })
                .flatten();
            if let Some(hook_hand) = hook_hand {
                self.tee_renderer.render_tee_hand(
                    &RenderTeeHandMath::new(&pos, RENDER_TEE_SIZE, &hook_hand),
                    &color_body,
                    skin(
                        character_info,
                        ninja_skin,
                        freeze_skin,
                        freezes,
                        ninjas,
                        skins,
                    ),
                    alpha,
                    &state,
                );
            }
        }
        // now render the tees & weapons
        for (character_id, player_render_info) in
            Self::render_info_iter(render_infos, own_character)
        {
            // dir to hook
            let pos = player_render_info.lerped_pos;

            let render_pos = pos;

            let vel = player_render_info.lerped_vel;
            let stationary = vel.x.abs() <= 1.0 / 32.0 / 256.0;
            let in_air = !collision.check_pointf(pos.x * 32.0, (pos.y + 0.5) * 32.0);
            let inactive = false; // TODO: m_pClient->m_aClients[ClientID].m_Afk || m_pClient->m_aClients[ClientID].m_Paused;
            let is_sit = inactive && !in_air && stationary;

            let vel_running = 5000.0 / 32.0 / 256.0;
            let input_dir = player_render_info.move_dir;
            let running = vel.x >= vel_running || vel.x <= -vel_running;
            let want_other_dir =
                (input_dir == -1 && vel.x > 0.0) || (input_dir == 1 && vel.x < 0.0); // TODO: use input?

            let is_freeze = player_render_info
                .debuffs
                .contains_key(&CharacterDebuff::Freeze);
            let is_ninja = player_render_info.buffs.contains_key(&CharacterBuff::Ninja);
            let is_ghost = player_render_info.buffs.contains_key(&CharacterBuff::Ghost);
            let should_render_weapon = !is_ninja && !is_ghost && !is_freeze;

            let character_info = character_infos.get(character_id);
            let freeze_skin = is_freeze.then(|| character_info.map(|char| &char.info.freeze));
            let ninja_skin = is_ninja.then(|| character_info.map(|char| &char.info.ninja));

            let weapon_hand = if should_render_weapon {
                let weapons = weapons.get_or_default_opt(character_info.map(|c| &c.info.weapon));
                self.toolkit_renderer.render_weapon_for_player(
                    weapons,
                    player_render_info,
                    render_pos,
                    ticks_in_a_second,
                    game_time_info,
                    state,
                    is_sit,
                    inactive,
                )
            } else if let Some(ninja_skin) = ninja_skin {
                self.toolkit_renderer.render_ninja_weapon(
                    ninjas.get_or_default_opt(ninja_skin),
                    particle_manager,
                    player_render_info,
                    game_time_info,
                    ticks_in_a_second,
                    **cur_time,
                    pos,
                    is_sit,
                    state,
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

            let mut anim_state = AnimState::default();
            anim_state.set(&base_anim(), &Duration::from_millis(0));

            // evaluate animation
            let walk_time = pos.x.rem_euclid(100.0 / 32.0) / (100.0 / 32.0);
            let run_time = pos.x.rem_euclid(200.0 / 32.0) / (200.0 / 32.0);

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

            let (color_body, color_feet) = skin_colors(character_info);

            let tee_render_info = TeeRenderInfo {
                color_body,
                color_feet,
                got_air_jump: player_render_info.has_air_jump,
                feet_flipped: false,
                size: RENDER_TEE_SIZE, // yes a tee is 2 tiles big (rendering wise)
                eye_left: player_render_info.left_eye,
                eye_right: player_render_info.right_eye,
            };

            let dir = normalize(&player_render_info.cursor_pos);
            let dir = vec2::new(dir.x as f32, dir.y as f32);

            self.tee_renderer.render_tee(
                &anim_state,
                skin(
                    character_info,
                    ninja_skin,
                    freeze_skin,
                    freezes,
                    ninjas,
                    skins,
                ),
                &tee_render_info,
                &TeeRenderHands {
                    left: None,
                    right: weapon_hand,
                },
                &dir,
                &render_pos,
                alpha,
                &state,
            );

            if let Some((emoticon_ticks, emoticon)) = player_render_info.emoticon {
                self.emoticon_renderer.render(&mut RenderEmoticonPipe {
                    emoticon_container: emoticons,
                    pos,
                    state: &state,
                    emoticon_key: character_info.map(|c| c.info.emoticons.borrow()),
                    emoticon,
                    emoticon_ticks,
                    intra_tick_time: game_time_info.intra_tick_time,
                    ticks_per_second: game_time_info.ticks_per_second,
                });
            }
        }
    }

    pub fn render_nameplates(
        &mut self,
        cur_time: &Duration,
        camera: &Camera,
        render_infos: &PoolLinkedHashMap<GameEntityId, CharacterRenderInfo>,
        character_infos: &PoolLinkedHashMap<GameEntityId, CharacterInfo>,
        nameplates: bool,
        own_nameplate: bool,
        own_character: Option<&GameEntityId>,
    ) {
        let state = self.base_state(camera);
        for (character_id, player_render_info) in
            Self::render_info_iter(render_infos, &own_character)
        {
            let pos = &player_render_info.lerped_pos;
            let character_info = character_infos.get(character_id);
            if let Some(name) = character_info
                .map(|c| c.info.name.as_str())
                .and_then(|n| (!n.is_empty()).then_some(n))
                .and_then(|n| {
                    (nameplates
                        && (own_nameplate || !own_character.is_some_and(|id| *id == *character_id)))
                    .then_some(n)
                })
            {
                self.nameplate_renderer.render(&mut NameplateRenderPipe {
                    cur_time,
                    name,
                    state: &state,
                    pos,
                    camera_zoom: camera.zoom,
                });
            }
        }
    }
}
