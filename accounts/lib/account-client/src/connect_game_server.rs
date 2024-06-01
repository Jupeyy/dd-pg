use accounts_base::{
    account_server::game_server_group::{
        GameServerKeyPairResponse, StoreGameServerKeyPairResponse,
    },
    client::{
        auth::prepare_auth_request,
        game_server_data::{
            decrypt_priv_key_from_main_secret, generate_game_server_group_data,
            ClientGameServerKeyPair, RequestGameServerKeyPair, RequestStoreGameServerKeyPair,
        },
        otp::OtpRequest,
    },
    game_server::server_id::GameServerGroupId,
};
use anyhow::anyhow;
use thiserror::Error;

use crate::{
    errors::{FsLikeError, HttpLikeError},
    interface::Io,
    machine_id::machine_uid,
    safe_interface::{IoSafe, SafeIo},
};

/// Errors related to a connecting to a game server.
#[derive(Error, Debug)]
pub enum ConnectGameServerError {
    /// Crypt related function failed to execute.
    #[error("A crypt function failed")]
    CryptFailed,
    /// A http like error occurred.
    #[error("{0}")]
    HttpLikeError(HttpLikeError),
    /// A std fs error occurred (e.g. reading files).
    #[error("{0}")]
    FsError(FsLikeError),
    /// User is _probably_ not logged in.
    /// Additionally:
    /// - For client: Account is not verified.
    /// - For server: Account is not verified;
    ///     Or the account is not a valid game server,
    ///     as in: is not verified as game server group.
    #[error("Not logged in (or account not verified).")]
    AuthInvalid,
    /// Server does not exist in the database,
    /// thus it's invalid to store a key pair for it.
    #[error("No permission to store this key pair for non-existing game server group.")]
    NoSuchGameServerGroup,
    /// Arbitrary error
    #[error("{0}")]
    Other(anyhow::Error),
}

impl From<HttpLikeError> for ConnectGameServerError {
    fn from(value: HttpLikeError) -> Self {
        Self::HttpLikeError(value)
    }
}

/// The client wants to connect to a game server of the given game server group.
/// - If a key-pair exists on the client then this simply reads and decrypts the
///     key-pair.
/// - If a key-pair does not exist on the client it:
///     - Downloads the current key-pair from the account server.
///     - Or if the account server has no such key-pair generates a key-pair
///         and stores it on the account server.
pub async fn connect_game_server(
    game_server_group_id: GameServerGroupId,
    main_secret: &[u8],
    io: &dyn Io,
) -> anyhow::Result<ClientGameServerKeyPair, ConnectGameServerError> {
    connect_game_server_impl(Some(game_server_group_id), main_secret, io.into()).await
}

pub(crate) async fn connect_game_server_impl(
    game_server_group_id: Option<GameServerGroupId>,
    main_secret: &[u8],
    io: IoSafe<'_>,
) -> anyhow::Result<ClientGameServerKeyPair, ConnectGameServerError> {
    let key_pair = io
        .read_game_server_group_key_pair_file(game_server_group_id)
        .await;

    let key_pair = key_pair
        .map_err(|_| ConnectGameServerError::CryptFailed)
        .and_then(|key_pair| {
            decrypt_priv_key_from_main_secret(key_pair.priv_key, main_secret)
                .map(|priv_key| (priv_key, key_pair.pub_key))
                .map_err(|_| ConnectGameServerError::CryptFailed)
        });
    match key_pair {
        Ok((private_key, public_key)) => Ok(ClientGameServerKeyPair {
            private_key,
            public_key,
        }),
        Err(_) => {
            // first requests the account server for a the key-pair
            let otps = io.request_otp(OtpRequest { count: 1 }).await?;
            let [otp] = otps.otps.try_into().map_err(|_| {
                ConnectGameServerError::Other(anyhow!("No one time password found in response."))
            })?;
            let mut session_key =
                io.read_session_key_pair_file()
                    .await
                    .map_err(|err| match err {
                        crate::errors::FsLikeError::Fs(err) => match err.kind() {
                            std::io::ErrorKind::NotFound => ConnectGameServerError::AuthInvalid,
                            _ => ConnectGameServerError::Other(err.into()),
                        },
                        crate::errors::FsLikeError::Other(err) => {
                            ConnectGameServerError::Other(err)
                        }
                    })?;
            let hashed_hw_id = machine_uid().map_err(|_| ConnectGameServerError::CryptFailed)?;
            let auth_data = prepare_auth_request(
                otp,
                hashed_hw_id,
                &mut session_key.priv_key,
                session_key.pub_key,
            );
            let key_pair = io
                .request_game_server_group_key_pair(RequestGameServerKeyPair {
                    auth_data,
                    game_server_group_id,
                })
                .await;

            let key_pair = key_pair
                .map_err(|_| ConnectGameServerError::CryptFailed)
                .and_then(|key_pair| match key_pair {
                    GameServerKeyPairResponse::Success(key_pair) => {
                        decrypt_priv_key_from_main_secret(key_pair.key_pair.priv_key, main_secret)
                            .map(|priv_key| (priv_key, key_pair.key_pair.pub_key))
                            .map_err(|_| ConnectGameServerError::CryptFailed)
                    }
                    GameServerKeyPairResponse::NotFound => {
                        Err(ConnectGameServerError::Other(anyhow!("no key pair found")))
                    }
                    GameServerKeyPairResponse::InvalidAuth => {
                        Err(ConnectGameServerError::AuthInvalid)
                    }
                });
            match key_pair {
                Ok((private_key, public_key)) => Ok(ClientGameServerKeyPair {
                    private_key,
                    public_key,
                }),
                Err(err) => {
                    if matches!(err, ConnectGameServerError::AuthInvalid) {
                        return Err(err);
                    }
                    // generate a new key-pair and store it on the account server
                    let key_pair = generate_game_server_group_data(main_secret)
                        .map_err(|_| ConnectGameServerError::CryptFailed)?;

                    let otps = io.request_otp(OtpRequest { count: 1 }).await?;
                    let [otp] = otps.otps.try_into().map_err(|_| {
                        ConnectGameServerError::Other(anyhow!(
                            "No one time password found in response."
                        ))
                    })?;
                    let mut session_key =
                        io.read_session_key_pair_file()
                            .await
                            .map_err(|err| match err {
                                crate::errors::FsLikeError::Fs(err) => match err.kind() {
                                    std::io::ErrorKind::NotFound => {
                                        ConnectGameServerError::AuthInvalid
                                    }
                                    _ => ConnectGameServerError::Other(err.into()),
                                },
                                crate::errors::FsLikeError::Other(err) => {
                                    ConnectGameServerError::Other(err)
                                }
                            })?;
                    let hashed_hw_id =
                        machine_uid().map_err(|_| ConnectGameServerError::CryptFailed)?;
                    let auth_data = prepare_auth_request(
                        otp,
                        hashed_hw_id,
                        &mut session_key.priv_key,
                        session_key.pub_key,
                    );

                    let store_res = io
                        .request_store_game_server_group_key_pair(RequestStoreGameServerKeyPair {
                            auth_data,
                            key_pair: key_pair.for_server,
                            game_server_group_id,
                        })
                        .await?;

                    match store_res {
                        StoreGameServerKeyPairResponse::Success => {
                            let private_key = key_pair.for_client.key_pair.priv_key.clone();
                            let public_key = key_pair.for_client.key_pair.pub_key;
                            let private_key =
                                decrypt_priv_key_from_main_secret(private_key, main_secret)
                                    .map_err(|_| ConnectGameServerError::CryptFailed)?;

                            io.write_game_server_group_key_pair_file(
                                game_server_group_id,
                                key_pair.for_client.key_pair,
                            )
                            .await
                            .map_err(ConnectGameServerError::FsError)?;

                            Ok(ClientGameServerKeyPair {
                                private_key,
                                public_key,
                            })
                        }
                        StoreGameServerKeyPairResponse::GameServerGroupNotFound => {
                            Err(ConnectGameServerError::NoSuchGameServerGroup)
                        }
                        StoreGameServerKeyPairResponse::InvalidAuth => {
                            Err(ConnectGameServerError::AuthInvalid)
                        }
                    }
                }
            }
        }
    }
}
