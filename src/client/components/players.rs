use arrayvec::ArrayString;
use graphics_types::{
    command_buffer::SRenderSpriteInfo,
    rendering::{ColorRGBA, ETextureIndex, State},
};

use crate::{
    client::component::{
        ComponentComponent, ComponentGameMsg, ComponentLoadIOPipe, ComponentLoadPipe,
        ComponentLoadWhileIOPipe, ComponentLoadable, ComponentRenderPipe, ComponentRenderable,
        ComponentUpdatable,
    },
    game::weapons::definitions::Weapons,
    render::{
        animation::AnimState,
        tee::{RenderTee, TeeEyeEmote, TeeRenderInfo, TeeRenderSkinTextures},
    },
};

use math::math::vector::{ubvec4, vec2};

use graphics::graphics::{
    GraphicsQuadContainerInterface, QuadContainerBuilder, QuadContainerIndex,
    QuadContainerRenderCount, SQuad,
};

/**
 * The player component renders all hooks
 * all weapons, and all players
 */
pub struct Players {
    quad_container_index: QuadContainerIndex,

    tee_renderer: Option<RenderTee>,

    hook_chain_texture: ETextureIndex,
    hook_head_texture: ETextureIndex,

    hook_chain_quad_offset: usize,
    hook_head_quad_offset: usize,

    weapon_textures: [ETextureIndex; 4], // TODO: NUM_WEAPONS
    weapon_quad_offsets: [usize; 4],     // TODO: NUM_WEAPONS
}

impl ComponentLoadable for Players {
    fn load_io(&mut self, io_pipe: &mut ComponentLoadIOPipe) {}

    fn init_while_io(&mut self, _pipe: &mut ComponentLoadWhileIOPipe) {}

    fn init(&mut self, pipe: &mut ComponentLoadPipe) -> Result<(), ArrayString<4096>> {
        self.tee_renderer = Some(RenderTee::new(pipe.graphics));

        self.quad_container_index = pipe
            .graphics
            .create_quad_container(&QuadContainerBuilder::new(false));

        (0..Weapons::NumWeapons as usize).for_each(|wi| {
            let mut quad = *SQuad::new().with_color(&ubvec4::new(255, 255, 255, 255));

            pipe.graphics
                .quad_container_add_quads(&self.quad_container_index, &[quad]);
        });

        pipe.graphics
            .quad_container_upload(&self.quad_container_index);
        /*
        m_WeaponEmoteQuadContainerIndex = Graphics()->CreateQuadContainer(false);

        Graphics()->SetColor(1.f, 1.f, 1.f, 1.f);

        for(int i = 0; i < NUM_WEAPONS; ++i)
        {
            float ScaleX, ScaleY;
            RenderTools()->GetSpriteScale(g_pData->m_Weapons.m_aId[i].m_pSpriteBody, ScaleX, ScaleY);
            Graphics()->QuadsSetSubset(0, 0, 1, 1);
            RenderTools()->QuadContainerAddSprite(m_WeaponEmoteQuadContainerIndex, g_pData->m_Weapons.m_aId[i].m_VisualSize * ScaleX, g_pData->m_Weapons.m_aId[i].m_VisualSize * ScaleY);
            Graphics()->QuadsSetSubset(0, 1, 1, 0);
            RenderTools()->QuadContainerAddSprite(m_WeaponEmoteQuadContainerIndex, g_pData->m_Weapons.m_aId[i].m_VisualSize * ScaleX, g_pData->m_Weapons.m_aId[i].m_VisualSize * ScaleY);
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
        Graphics()->QuadsSetRotation(0.f); */

        Ok(())
    }
}

impl ComponentUpdatable for Players {}

impl ComponentRenderable for Players {
    fn render(&mut self, pipe: &mut ComponentRenderPipe) {
        // first render the hooks
        // OLD: render everyone else's hook, then our own

        // intra tick
        // alpha other team
        // position (render pos)
        // hook (head, chain)
        // -> hand
        let players: [u8; 0] = [];
        players.iter().for_each(|p| {
            // render head
            let mut quad_scope = pipe.graphics.backend_handle.quad_scope_begin();
            quad_scope.set_texture(self.hook_head_texture);
            quad_scope.set_rotation(0.0);
            //Graphics()->QuadsSetRotation(angle(dir) + pi);
            quad_scope.set_colors_from_single(1.0, 1.0, 1.0, 0.0); //<-- alpha

            //Graphics()->SetColor(1.0f, 1.0f, 1.0f, Alpha);
            //Graphics()->RenderQuadContainerAsSprite(m_WeaponEmoteQuadContainerIndex, QuadOffset, HookPos.x, HookPos.y);

            pipe.graphics
                .quad_container_handle
                .RenderQuadContainerAsSprite(
                    &self.quad_container_index,
                    self.hook_head_quad_offset,
                    0.0,
                    0.0,
                    1.0,
                    1.0,
                    quad_scope,
                );

            // render chain
            let mut hook_chain_render_info: Vec<SRenderSpriteInfo> = Vec::new();
            let mut hook_chain_count = 0;
            let mut f = 24.0;
            let d = 0.0; // TODO
            while (f < d && hook_chain_count < 1024) {
                //let p = HookPos + dir * f;
                //s_aHookChainRenderInfo[HookChainCount].m_Pos[0] = p.x;
                //s_aHookChainRenderInfo[HookChainCount].m_Pos[1] = p.y;
                hook_chain_render_info[hook_chain_count].scale = 1.0;
                //s_aHookChainRenderInfo[HookChainCount].m_Rotation = angle(dir) + pi;
                // todo push

                f += 24.0;
                hook_chain_count += 1;
            }
            let mut quad_scope = pipe.graphics.backend_handle.quad_scope_begin();
            quad_scope.set_texture(self.hook_chain_texture);
            pipe.graphics
                .quad_container_handle
                .RenderQuadContainerAsSpriteMultiple(
                    &self.quad_container_index,
                    self.hook_chain_quad_offset,
                    &QuadContainerRenderCount::Count(hook_chain_count),
                    hook_chain_render_info,
                    quad_scope,
                );
        });

        // now render the weapons
        let mut quad_scope = pipe.graphics.backend_handle.quad_scope_begin();
        quad_scope.set_rotation(0.0); // TODO (State.GetAttach()->m_Angle * pi * 2 + Angle)

        // normal weapons
        //int CurrentWeapon = clamp(Player.m_Weapon, 0, NUM_WEAPONS - 1);
        let current_weapon = 0;
        quad_scope.set_texture(self.weapon_textures[current_weapon]);

        let quad_offset = self.weapon_quad_offsets[current_weapon];

        quad_scope.set_colors_from_single(1.0, 1.0, 1.0, 1.0); //<-- alpha

        let weapon: Weapons = Weapons::Gun; // TODO

        if weapon == Weapons::Hammer {
            // static position for hammer
            /*let WeaponPosition = Position + vec2(State.GetAttach()->m_X, State.GetAttach()->m_Y);
            WeaponPosition.y += g_pData->m_Weapons.m_aId[CurrentWeapon].m_Offsety;
            if(Direction.x < 0)
                WeaponPosition.x -= g_pData->m_Weapons.m_aId[CurrentWeapon].m_Offsetx;
            if(IsSit)
                WeaponPosition.y += 3.0f;

            // if active and attack is under way, bash stuffs
            if(!Inactive || LastAttackTime < m_pClient->m_aTuning[g_Config.m_ClDummy].GetWeaponFireDelay(Player.m_Weapon))
            {
                if(Direction.x < 0) {
                    quad_scope.set_rotation(-pi / 2 - State.GetAttach()->m_Angle * pi * 2);
                }
                else {
                    quad_scope.set_rotation(-pi / 2 + State.GetAttach()->m_Angle * pi * 2);
                }
            }
            else {
                quad_scope.set_rotation(Direction.x < 0 ? 100.0f : 500.0f);
            }

            pipe.graphics.quad_container_handle.RenderQuadContainerAsSprite(&self.quad_container_index
                , quad_offset,  WeaponPosition.x, WeaponPosition.y, 1.0, 1.0, quad_scope);*/
        }

        drop(quad_scope);

        /*
        vec2 dir = Direction;
        float Recoil = 0.0f;
        vec2 WeaponPosition;
        bool IsSit = Inactive && !InAir && Stationary;

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
        else
        {
            // TODO: should be an animation
            Recoil = 0;
            float a = AttackTicksPassed / 5.0f;
            if(a < 1)
                Recoil = sinf(a * pi);
            WeaponPosition = Position + dir * g_pData->m_Weapons.m_aId[CurrentWeapon].m_Offsetx - dir * Recoil * 10.0f;
            WeaponPosition.y += g_pData->m_Weapons.m_aId[CurrentWeapon].m_Offsety;
            if(IsSit)
                WeaponPosition.y += 3.0f;
            if(Player.m_Weapon == WEAPON_GUN && g_Config.m_ClOldGunPosition)
                WeaponPosition.y -= 8;
            Graphics()->RenderQuadContainerAsSprite(m_WeaponEmoteQuadContainerIndex, QuadOffset, WeaponPosition.x, WeaponPosition.y);
        }

        if(Player.m_Weapon == WEAPON_GUN || Player.m_Weapon == WEAPON_SHOTGUN)
        {
            // check if we're firing stuff
            if(g_pData->m_Weapons.m_aId[CurrentWeapon].m_NumSpriteMuzzles) // prev.attackticks)
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
            }
        }
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
        let mut state = State::new();

        let tee_render_info = TeeRenderInfo {
            render_skin: TeeRenderSkinTextures::Original(Default::default()),
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

        self.tee_renderer.as_mut().unwrap().render_tee(
            pipe.graphics,
            &AnimState {
                ..Default::default()
            },
            &tee_render_info,
            TeeEyeEmote::Normal,
            &vec2::new(0.0, 1.0),
            &vec2::new(0.0, 1.0),
            1.0,
            &state,
        );
    }
}

impl ComponentGameMsg for Players {}

impl ComponentComponent for Players {}

impl Players {
    pub fn new() -> Self {
        Self {
            quad_container_index: None,
            tee_renderer: None,

            hook_chain_texture: ETextureIndex::Invalid,
            hook_head_texture: ETextureIndex::Invalid,

            hook_chain_quad_offset: todo!(),
            hook_head_quad_offset: todo!(),
            weapon_textures: todo!(),
            weapon_quad_offsets: todo!(),
        }
    }
}
