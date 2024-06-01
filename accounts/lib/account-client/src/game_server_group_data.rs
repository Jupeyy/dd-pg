use accounts_base::client::game_server_data::ClientGameServerKeyPair;

use crate::{
    connect_game_server::{connect_game_server_impl, ConnectGameServerError},
    interface::Io,
};

/// The data for a game server that is a member of a game server group.
pub type GameServerGroupData = ClientGameServerKeyPair;

/// Errors related to a getting the server group data.
pub type GetGameServerGroupDataError = ConnectGameServerError;

/// The game server uses the account to show the clients that they are part of a game
/// server group, the game server thus works similar to the client and asks the account
/// server for a key-pair, which ultimately verifies group membership:
/// - If a key-pair exists on the account client then this simply reads and decrypts the
///     key-pair.
/// - If a key-pair does not exist on the account client it:
///     - Downloads the current key-pair from the account server.
///     - Or if the account server has no such key-pair generates a key-pair
///         and stores it on the account server.
pub async fn get_game_server_group_data(
    main_secret: &[u8],
    io: &dyn Io,
) -> anyhow::Result<GameServerGroupData, GetGameServerGroupDataError> {
    connect_game_server_impl(None, main_secret, io.into()).await
}
