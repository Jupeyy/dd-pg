use std::{collections::HashMap, ops::Range};

use anyhow::anyhow;
use command_parser::parser::{Command, Syn};
use game_interface::types::weapons::WeaponType;
use native::input::binds::{BindKey, KeyCode, PhysicalKey};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum BindActionsLocalPlayer {
    MoveLeft,
    MoveRight,
    Jump,
    Fire,
    Hook,
    NextWeapon,
    PrevWeapon,
    Weapon(WeaponType),
    OpenMenu,
    ActivateChatInput,
    ShowScoreboard,
    ShowChatHistory,
    ShowEmoteWheel,
    Kill,
    ToggleDummyCopyMoves,
    ToggleDummyHammerFly,
    VoteYes,
    VoteNo,
    ZoomOut,
    ZoomIn,
    ZoomReset,
}

const LOCAL_PLAYER_ACTIONS: [(&str, BindActionsLocalPlayer); 23] = [
    ("+left", BindActionsLocalPlayer::MoveLeft),
    ("+right", BindActionsLocalPlayer::MoveRight),
    ("+jump", BindActionsLocalPlayer::Jump),
    ("+fire", BindActionsLocalPlayer::Fire),
    ("+hook", BindActionsLocalPlayer::Hook),
    ("+nextweapon", BindActionsLocalPlayer::NextWeapon),
    ("+prevweapon", BindActionsLocalPlayer::PrevWeapon),
    // weapons
    (
        "+weapon1",
        BindActionsLocalPlayer::Weapon(WeaponType::Hammer),
    ),
    ("+weapon2", BindActionsLocalPlayer::Weapon(WeaponType::Gun)),
    (
        "+weapon3",
        BindActionsLocalPlayer::Weapon(WeaponType::Shotgun),
    ),
    (
        "+weapon4",
        BindActionsLocalPlayer::Weapon(WeaponType::Grenade),
    ),
    (
        "+weapon5",
        BindActionsLocalPlayer::Weapon(WeaponType::Laser),
    ),
    // weapons end
    ("ingame_menu", BindActionsLocalPlayer::OpenMenu),
    ("+show_chat", BindActionsLocalPlayer::ActivateChatInput),
    ("+scoreboard", BindActionsLocalPlayer::ShowScoreboard),
    ("vote_yes", BindActionsLocalPlayer::VoteYes),
    ("vote_no", BindActionsLocalPlayer::VoteNo),
    ("kill", BindActionsLocalPlayer::Kill),
    (
        "dummy_copy_moves",
        BindActionsLocalPlayer::ToggleDummyCopyMoves,
    ),
    (
        "dummy_hammer_fly",
        BindActionsLocalPlayer::ToggleDummyHammerFly,
    ),
    ("zoom-", BindActionsLocalPlayer::ZoomOut),
    ("zoom+", BindActionsLocalPlayer::ZoomIn),
    ("zoom", BindActionsLocalPlayer::ZoomReset),
];

pub fn gen_local_player_action_hash_map() -> HashMap<&'static str, BindActionsLocalPlayer> {
    LOCAL_PLAYER_ACTIONS.into_iter().collect()
}

pub fn gen_local_player_action_hash_map_rev() -> HashMap<BindActionsLocalPlayer, &'static str> {
    LOCAL_PLAYER_ACTIONS
        .into_iter()
        .map(|(v1, v2)| (v2, v1))
        .collect()
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum BindActionsHotkey {
    Screenshot,
    LocalConsole,
    RemoteConsole,
    ConsoleClose,
    DebugHud,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum BindActions {
    LocalPlayer(BindActionsLocalPlayer),
    Command(Command),
}

fn action_str_to_action(
    action_str: &str,
    map: &HashMap<&'static str, BindActionsLocalPlayer>,
) -> anyhow::Result<BindActionsLocalPlayer> {
    map.get(action_str)
        .cloned()
        .ok_or_else(|| anyhow!("not a valid action"))
}

fn action_to_action_str(
    action: BindActionsLocalPlayer,
    map: &HashMap<BindActionsLocalPlayer, &'static str>,
) -> anyhow::Result<&'static str> {
    map.get(&action)
        .cloned()
        .ok_or_else(|| anyhow!("not a valid action"))
}

fn bind_keys_str_to_bind_keys(bind_keys_str: &str) -> anyhow::Result<Vec<BindKey>> {
    let mut bind_keys: Vec<BindKey> = Vec::new();
    for bind_key_str in bind_keys_str.split('+') {
        let mut cap_bind_key_str = bind_key_str.to_string();
        cap_bind_key_str.make_ascii_lowercase();
        cap_bind_key_str = {
            let str_len = cap_bind_key_str.chars().count();
            let mut res: Vec<_> = cap_bind_key_str
                .chars()
                .enumerate()
                .collect::<Vec<(usize, char)>>()
                .windows(2)
                .flat_map(|arg| {
                    let [(_, c1), (c2_index, c2)] = [arg[0], arg[1]];
                    if c1.is_ascii_uppercase() {
                        vec![]
                    } else if c1 == '_' {
                        vec![c2.to_ascii_uppercase()]
                    } else if str_len - 1 == c2_index {
                        vec![c1, c2]
                    } else {
                        vec![c1]
                    }
                })
                .collect();
            if res.is_empty() {
                cap_bind_key_str.to_ascii_uppercase()
            } else {
                res[0] = res[0].to_ascii_uppercase();
                res.into_iter().collect()
            }
        };
        let bind_key_str = format!("\"{cap_bind_key_str}\"");
        if let Ok(key_code) = serde_json::from_str::<KeyCode>(&bind_key_str) {
            bind_keys.push(BindKey::Key(PhysicalKey::Code(key_code)));
        } else if let Ok(key_code) = serde_json::from_str::<_>(&bind_key_str) {
            bind_keys.push(BindKey::Mouse(key_code));
        } else if let Ok(key_code) = serde_json::from_str::<_>(&bind_key_str) {
            bind_keys.push(BindKey::Extra(key_code));
        } else {
            let bind_key_str = format!("\"Key{cap_bind_key_str}\"");
            if let Ok(key_code) = serde_json::from_str::<KeyCode>(&bind_key_str) {
                bind_keys.push(BindKey::Key(PhysicalKey::Code(key_code)));
            }
        }
    }
    anyhow::ensure!(!bind_keys.is_empty(), "no keys in bind found");
    Ok(bind_keys)
}

/// This is for a parsed console syntax
pub fn syn_to_bind(
    path: &[(Syn, Range<usize>)],
    map: &HashMap<&'static str, BindActionsLocalPlayer>,
) -> anyhow::Result<(Vec<BindKey>, Vec<BindActions>)> {
    let mut path = path.iter();

    let (keys_str, _) = path.next().ok_or_else(|| anyhow!("no keys text found"))?;
    let bind_keys = match keys_str {
        Syn::Text(keys_str) => bind_keys_str_to_bind_keys(keys_str)?,
        _ => anyhow::bail!("keys_str must be of type Text"),
    };

    let (action, _) = path.next().ok_or_else(|| anyhow!("no action text found"))?;

    let actions = match action {
        Syn::Commands(actions) => actions
            .iter()
            .map(|action| match action_str_to_action(&action.cmd_text, map) {
                Ok(action) => BindActions::LocalPlayer(action),
                Err(_) => BindActions::Command(action.clone()),
            })
            .collect(),
        act => anyhow::bail!("action must be of type Text, but was {:?}", act),
    };

    Ok((bind_keys, actions))
}

/// Expects a well formatted string previously created by [`bind_to_str`].
pub fn str_to_bind(
    bind_str: &str,
    map: &HashMap<&'static str, BindActionsLocalPlayer>,
) -> (Vec<BindKey>, BindActionsLocalPlayer) {
    let [_, bind_keys_str, action_str]: [&str; 3] = bind_str
        .splitn(3, ' ')
        .collect::<Vec<&str>>()
        .try_into()
        .unwrap();

    let bind_keys = bind_keys_str_to_bind_keys(bind_keys_str).unwrap();

    let action = action_str_to_action(action_str, map).unwrap();

    (bind_keys, action)
}

pub fn bind_to_str(
    bind_keys: &[BindKey],
    actions: Vec<BindActions>,
    map: &HashMap<BindActionsLocalPlayer, &'static str>,
) -> String {
    let mut res = "bind ".to_string();

    fn replace_inner_upper_with_underscore(s: &str) -> String {
        s.chars()
            .enumerate()
            .flat_map(|(index, c)| {
                if index != 0 && c.is_ascii_uppercase() {
                    vec!['_', c]
                } else {
                    vec![c]
                }
            })
            .collect()
    }

    let key_chain_len = bind_keys.len();
    for (index, bind_key) in bind_keys.iter().enumerate() {
        match bind_key {
            BindKey::Key(key) => match key {
                PhysicalKey::Code(key) => {
                    res.push_str(
                        replace_inner_upper_with_underscore(
                            &serde_json::to_string(key)
                                .unwrap()
                                .replace("Key", "")
                                .replace('"', ""),
                        )
                        .to_lowercase()
                        .as_str(),
                    );
                }
                PhysicalKey::Unidentified(_) => {
                    // ignore
                }
            },
            BindKey::Mouse(btn) => {
                res.push_str(
                    replace_inner_upper_with_underscore(
                        &serde_json::to_string(btn).unwrap().replace('"', ""),
                    )
                    .to_lowercase()
                    .as_str(),
                );
            }
            BindKey::Extra(ext) => {
                res.push_str(
                    replace_inner_upper_with_underscore(
                        &serde_json::to_string(ext).unwrap().replace('"', ""),
                    )
                    .to_lowercase()
                    .as_str(),
                );
            }
        }

        if index + 1 != key_chain_len {
            res.push('+');
        }
    }

    res.push(' ');

    let actions_str = actions
        .into_iter()
        .map(|action| match action {
            BindActions::LocalPlayer(action) => {
                action_to_action_str(action, map).unwrap().to_string()
            }
            BindActions::Command(cmd) => cmd.to_string(),
        })
        .collect::<Vec<_>>()
        .join(";");

    res.push_str(&actions_str);

    res
}

#[cfg(test)]
mod test {

    use native::input::binds::{BindKey, KeyCode, MouseButton, MouseExtra, PhysicalKey};

    use crate::binds::{
        bind_to_str, gen_local_player_action_hash_map_rev, BindActions, BindActionsLocalPlayer,
    };

    #[test]
    fn bind_json_abuses() {
        let map = gen_local_player_action_hash_map_rev();
        assert!(bind_to_str(
            &[BindKey::Key(PhysicalKey::Code(KeyCode::KeyT))],
            vec![BindActions::LocalPlayer(
                BindActionsLocalPlayer::ActivateChatInput
            )],
            &map
        )
        .contains("bind t "));
        assert!(bind_to_str(
            &[
                BindKey::Key(PhysicalKey::Code(KeyCode::ControlLeft)),
                BindKey::Key(PhysicalKey::Code(KeyCode::KeyT))
            ],
            vec![BindActions::LocalPlayer(
                BindActionsLocalPlayer::ActivateChatInput
            )],
            &map
        )
        .contains("bind control_left+t "));
        assert!(bind_to_str(
            &[BindKey::Mouse(MouseButton::Left)],
            vec![BindActions::LocalPlayer(BindActionsLocalPlayer::Fire)],
            &map
        )
        .contains("bind left "));
        assert!(bind_to_str(
            &[BindKey::Extra(MouseExtra::WheelDown)],
            vec![BindActions::LocalPlayer(BindActionsLocalPlayer::PrevWeapon)],
            &map
        )
        .contains("bind wheel_down "));
    }
}
