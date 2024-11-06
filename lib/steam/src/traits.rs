use async_trait::async_trait;

/// The functionality of the steam client
#[async_trait]
pub trait SteamClient: Send + Sync {
    fn is_stub(&self) -> bool;

    fn steam_id64(&self) -> i64;
    fn steam_user_name(&self) -> String;

    async fn session_ticket_for_webapi(&self) -> anyhow::Result<Vec<u8>>;
}

/// The underlaying object is a RAII object
/// and must be kept alive during the whole runtime
/// of the app.
pub trait SteamRaii {}
