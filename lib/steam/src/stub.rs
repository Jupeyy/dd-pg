use anyhow::anyhow;
use async_trait::async_trait;

use crate::traits::{SteamClient, SteamRaii};

pub struct SteamSt;
impl SteamRaii for SteamSt {}

pub struct SteamMt;
#[async_trait]
impl SteamClient for SteamMt {
    fn is_stub(&self) -> bool {
        true
    }

    fn steam_id64(&self) -> i64 {
        -1
    }

    fn steam_user_name(&self) -> String {
        "invalid".to_string()
    }

    async fn session_ticket_for_webapi(&self) -> anyhow::Result<Vec<u8>> {
        Err(anyhow!("This is just a stub."))
    }
}
