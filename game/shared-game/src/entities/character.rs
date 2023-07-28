pub mod character {
    use std::collections::HashMap;

    use shared_base::{
        game_types::TGameElementID,
        network::messages::MsgObjPlayerInput,
        reuseable::{CloneWithCopyableElements, ReusableCore},
        types::GameTickType,
    };

    use crate::{
        entities::{
            character_core::character_core::{Core, CorePhysics, CorePipe, CoreReusable},
            entity::entity::{EntitiyEvent, Entity, EntityInterface},
        },
        player::player::Player,
        simulation_pipe::simulation_pipe::{
            SimulationPipeCharacter, SimulationPipeCharactersGetter,
        },
        weapons::definitions::weapon_def::{Weapon, WeaponType},
    };

    use bincode::{Decode, Encode};
    use hashlink::LinkedHashMap;
    use math::math::{lerp, normalize, vector::vec2};
    use pool::{
        datatypes::PoolLinkedHashMap, mt_recycle::Recycle as MtRecycle, pool::Pool,
        recycle::Recycle, traits::Recyclable,
    };
    use serde::{Deserialize, Serialize};

    pub const PHYSICAL_SIZE: f32 = 28.0;
    pub const RECOIL_TIME: GameTickType = 6;

    #[derive(Serialize, Deserialize, Copy, Clone, Default, Encode, Decode)]
    pub struct CharacterCore {
        pub core: Core,
        // vanilla
        pub active_weapon: WeaponType,
        pub prev_weapon: WeaponType,
        pub queued_weapon: Option<WeaponType>,
        pub health: u32,
        pub shields: u32,
        pub recoil_start_tick: GameTickType,
    }

    #[derive(Serialize, Deserialize, Clone, Encode, Decode)]
    pub struct CharacterReusableCore {
        pub core: CoreReusable,
        pub weapons: HashMap<WeaponType, Weapon>,
    }

    impl CloneWithCopyableElements for CharacterReusableCore {
        fn copy_clone_from(&mut self, other: &Self) {
            self.core.copy_clone_from(&other.core);
            self.weapons.copy_clone_from(&other.weapons);
        }
    }

    impl Recyclable for CharacterReusableCore {
        fn new() -> Self {
            Self {
                core: CoreReusable::new(),
                weapons: Default::default(),
            }
        }
        fn reset(&mut self) {
            self.core.reset();
            self.weapons.reset();
        }
    }

    impl ReusableCore for CharacterReusableCore {}

    pub type PoolCharacterReusableCore = Recycle<CharacterReusableCore>;
    pub type MtPoolCharacterReusableCore = MtRecycle<CharacterReusableCore>;

    pub struct CharacterPool {
        pub(crate) character_pool: Pool<PoolCharacters>,
        pub(crate) character_reusable_cores_pool: Pool<CharacterReusableCore>,
    }

    pub struct Character {
        pub base: Entity,
        cores: [CharacterCore; 2],
        reusable_cores: [PoolCharacterReusableCore; 2],
    }

    impl Character {
        pub fn new(game_el_id: &TGameElementID, character_pool: &mut CharacterPool) -> Self {
            Self {
                base: Entity::new(game_el_id),
                cores: Default::default(),
                reusable_cores: [
                    character_pool.character_reusable_cores_pool.new(),
                    character_pool.character_reusable_cores_pool.new(),
                ],
            }
        }

        pub fn lerp_core_pos(&self, amount: f64) -> vec2 {
            lerp(
                &self.cores[0].core.pos,
                &self.cores[1].core.pos,
                amount as f32,
            )
        }

        pub fn lerp_core_vel(&self, amount: f64) -> vec2 {
            lerp(
                &self.cores[0].core.vel,
                &self.cores[1].core.vel,
                amount as f32,
            )
        }

        pub fn lerp_core_hook_pos(&self, amount: f64) -> vec2 {
            lerp(
                &self.cores[0].core.hook_pos,
                &self.cores[1].core.hook_pos,
                amount as f32,
            )
        }

        fn die(
            ent: &mut Entity,
            core: &CharacterCore,
            cur_tick: GameTickType,
            ticks_in_a_second: GameTickType,
        ) {
            ent.entity_events.push(EntitiyEvent::Die {
                pos: core.core.pos,
                respawns_at_tick: Some(cur_tick + ticks_in_a_second / 2),
            });

            //int ModeSpecial = GameServer()->m_pController->OnCharacterDeath(this, (Killer < 0) ? 0 : GameServer()->m_apPlayers[Killer], Weapon);

            /*char aBuf[256];
            if(Killer < 0)
            {
                /*str_format(aBuf, sizeof(aBuf), "kill killer='%d:%d:' victim='%d:%d:%s' weapon=%d special=%d",
                    Killer, - 1 - Killer,
                    m_pPlayer->GetCID(), m_pPlayer->GetTeam(), Server()->ClientName(m_pPlayer->GetCID()), Weapon, ModeSpecial
                );*/
            }
            else
            {
                /*str_format(aBuf, sizeof(aBuf), "kill killer='%d:%d:%s' victim='%d:%d:%s' weapon=%d special=%d",
                    Killer, GameServer()->m_apPlayers[Killer]->GetTeam(), Server()->ClientName(Killer),
                    m_pPlayer->GetCID(), m_pPlayer->GetTeam(), Server()->ClientName(m_pPlayer->GetCID()), Weapon, ModeSpecial
                );*/
            }*/
            //GameServer()->Console()->Print(IConsole::OUTPUT_LEVEL_DEBUG, "game", aBuf);
            /*
                   // send the kill message
                   CNetMsg_Sv_KillMsg Msg;
                   Msg.m_Victim = m_pPlayer->GetCID();
                   Msg.m_ModeSpecial = ModeSpecial;
                   for(int i = 0 ; i < MAX_CLIENTS; i++)
                   {
                       if(!Server()->ClientIngame(i))
                           continue;

                       if(Killer < 0 && Server()->GetClientVersion(i) < MIN_KILLMESSAGE_CLIENTVERSION)
                       {
                           Msg.m_Killer = 0;
                           Msg.m_Weapon = WEAPON_WORLD;
                       }
                       else
                       {
                           Msg.m_Killer = Killer;
                           Msg.m_Weapon = Weapon;
                       }
                       Server()->SendPackMsg(&Msg, MSGFLAG_VITAL, i);
                   }

                   // a nice sound
                   GameServer()->CreateSound(m_Pos, SOUND_PLAYER_DIE);
            */
        }

        fn set_weapon(
            ent: &mut Entity,
            core: &mut CharacterCore,
            reusable_core: &mut CharacterReusableCore,
            new_weapon: WeaponType,
        ) {
            if core.active_weapon == new_weapon {
                return;
            }

            core.prev_weapon = core.active_weapon;
            core.queued_weapon = None;
            core.active_weapon = new_weapon;
            ent.entity_events.push(EntitiyEvent::Sound {});
            // TODO: GameServer()->CreateSound(m_Pos, SOUND_WEAPON_SWITCH);

            if core.active_weapon >= WeaponType::NumWeapons {
                core.active_weapon = WeaponType::Invalid;
            }
            if let Some(_) = reusable_core.weapons.get_mut(&core.active_weapon) {
                // TODO: weapon.next_ammo_regeneration_tick
                //core.weapons[m_ActiveWeapon].m_AmmoRegenStart = -1;
            }
        }

        fn do_weapon_switch(
            ent: &mut Entity,
            core: &mut CharacterCore,
            reusable_core: &mut CharacterReusableCore,
            cur_tick: GameTickType,
        ) {
            // make sure we can switch
            if (cur_tick - core.recoil_start_tick <= RECOIL_TIME) || core.queued_weapon.is_none()
            // TODO: ninja || reusable_core.weapons.contains_key(k) m_aWeapons[WEAPON_NINJA].m_Got
            {
                return;
            }

            // switch weapon
            Self::set_weapon(ent, core, reusable_core, core.queued_weapon.unwrap());
        }

        pub fn take_damage(
            &mut self,
            core_index: usize,
            Force: &vec2,
            Source: &vec2,
            DmgAmount: i32,
            From: i32,
            Weapon: WeaponType,
        ) -> bool {
            /*m_Core.m_Vel += Force;

            if(From >= 0)
            {
                if(GameServer()->m_pController->IsFriendlyFire(m_pPlayer->GetCID(), From))
                    return false;
            }
            else
            {
                int Team = TEAM_RED;
                if(From == PLAYER_TEAM_BLUE)
                    Team = TEAM_BLUE;
                if(GameServer()->m_pController->IsFriendlyTeamFire(m_pPlayer->GetTeam(), Team))
                    return false;
            }

            // m_pPlayer only inflicts half damage on self
            if(From == m_pPlayer->GetCID())
                Dmg = maximum(1, Dmg/2);

            int OldHealth = m_Health, OldArmor = m_Armor;
            if(Dmg)
            {
                if(m_Armor)
                {
                    if(Dmg > 1)
                    {
                        m_Health--;
                        Dmg--;
                    }

                    if(Dmg > m_Armor)
                    {
                        Dmg -= m_Armor;
                        m_Armor = 0;
                    }
                    else
                    {
                        m_Armor -= Dmg;
                        Dmg = 0;
                    }
                }

                m_Health -= Dmg;
            }

            // create healthmod indicator
            GameServer()->CreateDamage(m_Pos, m_pPlayer->GetCID(), Source, OldHealth-m_Health, OldArmor-m_Armor, From == m_pPlayer->GetCID());

            // do damage Hit sound
            if(From >= 0 && From != m_pPlayer->GetCID() && GameServer()->m_apPlayers[From])
            {
                int64 Mask = CmaskOne(From);
                for(int i = 0; i < MAX_CLIENTS; i++)
                {
                    if(GameServer()->m_apPlayers[i] && (GameServer()->m_apPlayers[i]->GetTeam() == TEAM_SPECTATORS ||  GameServer()->m_apPlayers[i]->m_DeadSpecMode) &&
                        GameServer()->m_apPlayers[i]->GetSpectatorID() == From)
                        Mask |= CmaskOne(i);
                }
                GameServer()->CreateSound(GameServer()->m_apPlayers[From]->m_ViewPos, SOUND_HIT, Mask);
            }

            // check for death
            if(m_Health <= 0)
            {
                Die(From, Weapon);

                // set attacker's face to happy (taunt!)
                if(From >= 0 && From != m_pPlayer->GetCID() && GameServer()->m_apPlayers[From])
                {
                    CCharacter *pChr = GameServer()->m_apPlayers[From]->GetCharacter();
                    if(pChr)
                    {
                        pChr->SetEmote(EMOTE_HAPPY, Server()->Tick() + Server()->TickSpeed());
                    }
                }

                return false;
            }

            if(Dmg > 2)
                GameServer()->CreateSound(m_Pos, SOUND_PLAYER_PAIN_LONG);
            else
                GameServer()->CreateSound(m_Pos, SOUND_PLAYER_PAIN_SHORT);

            SetEmote(EMOTE_PAIN, Server()->Tick() + 500 * Server()->TickSpeed() / 1000);

            return true;*/
            true
        }

        fn fire_weapon(
            player: &Player,
            ent: &mut Entity,
            core: &mut CharacterCore,
            reusable_core: &mut CharacterReusableCore,
            cur_tick: GameTickType,
            ticks_in_a_second: GameTickType,
        ) {
            if (cur_tick - core.recoil_start_tick <= RECOIL_TIME) {
                return;
            }

            Self::do_weapon_switch(ent, core, reusable_core, cur_tick);
            let input = &player.input.inp;
            let cursor_pos = input.cursor.to_vec2();
            let direction = normalize(&vec2::new(cursor_pos.x as f32, cursor_pos.y as f32));

            let full_auto = if core.active_weapon == WeaponType::Grenade
                || core.active_weapon == WeaponType::Shotgun
                || core.active_weapon == WeaponType::Laser
            {
                true
            } else {
                false
            };

            // check if we gonna fire
            // TODO: bool or this ? (CountInput(m_LatestPrevInput.m_Fire, m_LatestInput.m_Fire).m_Presses)
            let will_fire = input.fire;

            if !will_fire {
                return;
            }

            // check for ammo
            let cur_weapon = reusable_core.weapons.get(&core.active_weapon);
            if cur_weapon.is_some() && cur_weapon.unwrap().cur_ammo == 0 {
                // 125ms is a magical limit of how fast a human can click
                //m_ReloadTimer = 125 * Server()->TickSpeed() / 1000;
                /* TODO: if(m_LastNoAmmoSound+Server()->TickSpeed() <= Server()->Tick())
                {
                    GameServer()->CreateSound(m_Pos, SOUND_WEAPON_NOAMMO);
                    m_LastNoAmmoSound = Server()->Tick();
                }*/
                return;
            }

            let proj_start_pos = core.core.pos + direction * PHYSICAL_SIZE * 0.75;

            /* TODO: is this really needed?
            if(Config()->m_Debug)
            {
                char aBuf[256];
                str_format(aBuf, sizeof(aBuf), "shot player='%d:%s' team=%d weapon=%d", m_pPlayer->GetCID(), Server()->ClientName(m_pPlayer->GetCID()), m_pPlayer->GetTeam(), m_ActiveWeapon);
                GameServer()->Console()->Print(IConsole::OUTPUT_LEVEL_DEBUG, "game", aBuf);
            }*/

            match core.active_weapon {
                WeaponType::Hammer => {
                    // TODO: GameServer()->CreateSound(m_Pos, SOUND_HAMMER_FIRE);

                    /*CCharacter *apEnts[MAX_CLIENTS];
                    int Hits = 0;
                    int Num = GameWorld()->FindEntities(ProjStartPos, GetProximityRadius()*0.5f, (CEntity**)apEnts,
                                                                MAX_CLIENTS, CGameWorld::ENTTYPE_CHARACTER);

                    for(int i = 0; i < Num; ++i)
                    {
                        CCharacter *pTarget = apEnts[i];

                        if((pTarget == this) || GameServer()->Collision()->IntersectLine(ProjStartPos, pTarget->m_Pos, NULL, NULL))
                            continue;

                        // set his velocity to fast upward (for now)
                        if(length(pTarget->m_Pos-ProjStartPos) > 0.0f)
                            GameServer()->CreateHammerHit(pTarget->m_Pos-normalize(pTarget->m_Pos-ProjStartPos)*GetProximityRadius()*0.5f);
                        else
                            GameServer()->CreateHammerHit(ProjStartPos);

                        vec2 Dir;
                        if(length(pTarget->m_Pos - m_Pos) > 0.0f)
                            Dir = normalize(pTarget->m_Pos - m_Pos);
                        else
                            Dir = vec2(0.f, -1.f);

                        pTarget->TakeDamage(vec2(0.f, -1.f) + normalize(Dir + vec2(0.f, -1.1f)) * 10.0f, Dir*-1, g_pData->m_Weapons.m_Hammer.m_pBase->m_Damage,
                            m_pPlayer->GetCID(), m_ActiveWeapon);
                        Hits++;
                    }

                    // if we Hit anything, we have to wait for the reload
                    if(Hits)
                        m_ReloadTimer = Server()->TickSpeed()/3;*/
                }
                WeaponType::Gun => {
                    ent.entity_events.push(EntitiyEvent::Projectile {
                        pos: proj_start_pos,
                        dir: direction,
                    });
                    /*new CProjectile(GameWorld(), WEAPON_GUN,
                        m_pPlayer->GetCID(),
                        ProjStartPos,
                        Direction,
                        (int)(Server()->TickSpeed()*GameServer()->Tuning()->m_GunLifetime),
                        g_pData->m_Weapons.m_Gun.m_pBase->m_Damage, false, 0, -1, WEAPON_GUN);

                    GameServer()->CreateSound(m_Pos, SOUND_GUN_FIRE);*/
                }
                WeaponType::Shotgun => {
                    /*int ShotSpread = 2;

                    for(int i = -ShotSpread; i <= ShotSpread; ++i)
                    {
                        float Spreading[] = {-0.185f, -0.070f, 0, 0.070f, 0.185f};
                        float a = angle(Direction);
                        a += Spreading[i+2];
                        float v = 1-(absolute(i)/(float)ShotSpread);
                        float Speed = mix((float)GameServer()->Tuning()->m_ShotgunSpeeddiff, 1.0f, v);
                        new CProjectile(GameWorld(), WEAPON_SHOTGUN,
                            m_pPlayer->GetCID(),
                            ProjStartPos,
                            vec2(cosf(a), sinf(a))*Speed,
                            (int)(Server()->TickSpeed()*GameServer()->Tuning()->m_ShotgunLifetime),
                            g_pData->m_Weapons.m_Shotgun.m_pBase->m_Damage, false, 0, -1, WEAPON_SHOTGUN);
                    }

                    GameServer()->CreateSound(m_Pos, SOUND_SHOTGUN_FIRE);*/
                }
                WeaponType::Grenade => {
                    /*new CProjectile(GameWorld(), WEAPON_GRENADE,
                        m_pPlayer->GetCID(),
                        ProjStartPos,
                        Direction,
                        (int)(Server()->TickSpeed()*GameServer()->Tuning()->m_GrenadeLifetime),
                        g_pData->m_Weapons.m_Grenade.m_pBase->m_Damage, true, 0, SOUND_GRENADE_EXPLODE, WEAPON_GRENADE);

                    GameServer()->CreateSound(m_Pos, SOUND_GRENADE_FIRE);*/
                }
                WeaponType::Laser => {
                    /*new CLaser(GameWorld(), m_Pos, Direction, GameServer()->Tuning()->m_LaserReach, m_pPlayer->GetCID());
                    GameServer()->CreateSound(m_Pos, SOUND_LASER_FIRE);*/
                }
                /*case WEAPON_NINJA:
                {
                    m_NumObjectsHit = 0;

                    m_Ninja.m_ActivationDir = Direction;
                    m_Ninja.m_CurrentMoveTime = g_pData->m_Weapons.m_Ninja.m_Movetime * Server()->TickSpeed() / 1000;
                    m_Ninja.m_OldVelAmount = length(m_Core.m_Vel);

                    GameServer()->CreateSound(m_Pos, SOUND_NINJA_FIRE);
                } break;*/
                _ => panic!("fire weapon is not implemented for this weapon"),
            }

            //core.m_AttackTick = cur_tick;

            //if(m_aWeapons[m_ActiveWeapon].m_Ammo > 0) // -1 == unlimited
            //    m_aWeapons[m_ActiveWeapon].m_Ammo--;

            //if(!m_ReloadTimer)
            //    m_ReloadTimer = g_pData->m_Weapons.m_aId[m_ActiveWeapon].m_Firedelay * Server()->TickSpeed() / 1000;
        }

        fn handle_weapons(
            player: &Player,
            ent: &mut Entity,
            core: &mut CharacterCore,
            reusable_core: &mut CharacterReusableCore,
            cur_tick: GameTickType,
            ticks_in_a_second: GameTickType,
        ) {
            //ninja
            // TODO: HandleNinja();

            // check reload timer
            if core.recoil_start_tick > 0 {
                core.recoil_start_tick -= 1;
                return;
            }

            // fire Weapon, if wanted
            Self::fire_weapon(
                player,
                ent,
                core,
                reusable_core,
                cur_tick,
                ticks_in_a_second,
            );

            // ammo regen
            /*int AmmoRegenTime = g_pData->m_Weapons.m_aId[m_ActiveWeapon].m_Ammoregentime;
            if(AmmoRegenTime && m_aWeapons[m_ActiveWeapon].m_Ammo >= 0)
            {
                // If equipped and not active, regen ammo?
                if(m_ReloadTimer <= 0)
                {
                    if(m_aWeapons[m_ActiveWeapon].m_AmmoRegenStart < 0)
                        m_aWeapons[m_ActiveWeapon].m_AmmoRegenStart = Server()->Tick();

                    if((Server()->Tick() - m_aWeapons[m_ActiveWeapon].m_AmmoRegenStart) >= AmmoRegenTime * Server()->TickSpeed() / 1000)
                    {
                        // Add some ammo
                        m_aWeapons[m_ActiveWeapon].m_Ammo = minimum(m_aWeapons[m_ActiveWeapon].m_Ammo + 1,
                            g_pData->m_Weapons.m_aId[m_ActiveWeapon].m_Maxammo);
                        m_aWeapons[m_ActiveWeapon].m_AmmoRegenStart = -1;
                    }
                }
                else
                {
                    m_aWeapons[m_ActiveWeapon].m_AmmoRegenStart = -1;
                }
            }*/
        }
    }

    pub struct CorePipeStr<'a> {
        pub input: &'a MsgObjPlayerInput,
        pub cur_core_index: usize,
        characters: &'a mut dyn SimulationPipeCharactersGetter,
        character_id: &'a TGameElementID,
    }

    impl<'a> CorePipe for CorePipeStr<'a> {
        fn get_input_copy(&self) -> MsgObjPlayerInput {
            *self.input
        }

        fn tick_speed(&self) -> GameTickType {
            50 // TODO
        }

        fn get_core(&mut self) -> &mut Core {
            &mut self
                .characters
                .get_character()
                .get_core_at_index_mut(self.cur_core_index)
                .core
        }

        fn get_core_and_reusable_core(&mut self) -> (&mut Core, &mut CoreReusable) {
            let (_, core, reusable_core) = self
                .characters
                .get_character()
                .split_mut(self.cur_core_index);
            (&mut core.core, &mut reusable_core.core)
        }

        fn get_other_character_id_and_cores_iter(
            &self,
            for_each_func: &mut dyn FnMut(&TGameElementID, &Core),
        ) {
            self.characters
                .get_other_character_id_and_cores_iter(for_each_func)
        }

        fn get_other_character_id_and_cores_iter_mut(
            &mut self,
            for_each_func: &mut dyn FnMut(&TGameElementID, &mut Core, &mut CoreReusable),
        ) {
            self.characters
                .get_other_character_id_and_cores_iter_mut(for_each_func)
        }

        fn get_other_character_core_by_id(&self, other_char_id: &TGameElementID) -> &Core {
            self.characters
                .get_other_character_core_by_id(other_char_id)
        }

        fn set_or_reset_hooked_char(&mut self, id: Option<TGameElementID>) {
            let char = self.characters.get_character();
            let cur_id = char.reusable_cores[self.cur_core_index]
                .core
                .hooked_character
                .id;
            char.reusable_cores[self.cur_core_index]
                .core
                .hooked_character
                .id = id.clone();
            // if the player was attached to smth, deattach it
            if let Some(cur_id) = cur_id {
                let char = self.characters.get_other_character_by_id_mut(&cur_id);
                char.reusable_cores[self.cur_core_index]
                    .core
                    .hooked_character
                    .attached_characters_ids
                    .remove(&cur_id);
            }
            // if the new id is attaching to a player, add it to their list
            if let Some(id) = id {
                let char = self.characters.get_other_character_by_id_mut(&id);
                char.reusable_cores[self.cur_core_index]
                    .core
                    .hooked_character
                    .attached_characters_ids
                    .insert(self.character_id.clone());
            }
        }
    }

    impl CorePhysics for Character {}

    impl<'a> EntityInterface<CharacterCore, CharacterReusableCore, SimulationPipeCharacter<'a>>
        for Character
    {
        fn pre_tick(_pipe: &mut SimulationPipeCharacter) {}

        fn tick(pipe: &mut SimulationPipeCharacter) {
            let mut core_pipe = CorePipeStr {
                input: pipe
                    .player_inputs
                    .get_input(&pipe.character_player.id)
                    .unwrap(),
                characters: pipe.characters,
                cur_core_index: pipe.cur_core_index,
                character_id: &pipe.character_player.character_info.character_id,
            };
            Self::physics_tick(true, true, &mut core_pipe, pipe.collision);

            let (cur_tick, ticks_in_a_second) = (pipe.cur_tick, 50 /* TODO: */);
            let (ent, core, reusable_core, cur_tick, collision, player) = pipe.get_split_mut();

            if Entity::outside_of_playfield(&core.core.pos, collision) {
                Self::die(ent, core, cur_tick, ticks_in_a_second);
                return;
            }

            Self::handle_weapons(
                player,
                ent,
                core,
                reusable_core,
                cur_tick,
                ticks_in_a_second,
            );
        }

        fn tick_deferred(pipe: &mut SimulationPipeCharacter) {
            let mut core_pipe = CorePipeStr {
                input: pipe
                    .player_inputs
                    .get_input(&pipe.character_player.id)
                    .unwrap(),
                characters: pipe.characters,
                cur_core_index: pipe.cur_core_index,
                character_id: &pipe.character_player.character_info.character_id,
            };
            Self::physics_move(&mut core_pipe, &pipe.collision);
            Self::physics_quantize(&mut pipe.get_ent_and_core_mut().1.core);
        }

        fn split_mut(
            self: &mut Self,
            index: usize,
        ) -> (
            &mut Entity,
            &mut CharacterCore,
            &mut PoolCharacterReusableCore,
        ) {
            (
                &mut self.base,
                &mut self.cores[index],
                &mut self.reusable_cores[index],
            )
        }

        fn get_core_at_index(&self, index: usize) -> &CharacterCore {
            &self.cores[index]
        }

        fn get_core_at_index_mut(&mut self, index: usize) -> &mut CharacterCore {
            &mut self.cores[index]
        }

        fn get_reusable_cores_mut(&mut self) -> &mut [PoolCharacterReusableCore] {
            self.reusable_cores.as_mut_slice()
        }

        fn get_reusable_core_at_index(&self, index: usize) -> &PoolCharacterReusableCore {
            &self.reusable_cores[index]
        }

        fn get_reusable_core_at_index_mut(
            &mut self,
            index: usize,
        ) -> &mut PoolCharacterReusableCore {
            &mut self.reusable_cores[index]
        }
    }

    pub type PoolCharacters = LinkedHashMap<TGameElementID, Character>;
    pub type Characters = PoolLinkedHashMap<TGameElementID, Character>;
}
