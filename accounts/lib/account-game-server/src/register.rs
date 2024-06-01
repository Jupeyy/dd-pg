pub(crate) mod queries;

use std::sync::Arc;

use account_client::{
    errors::HttpLikeError, interface::Io, register_token::get_account_id_of_register_token,
};
use account_sql::query::Query;
use accounts_base::{account_server::register_token::RegisterToken, game_server::user_id::UserId};
use sqlx::Acquire;
use thiserror::Error;

use crate::shared::Shared;

use self::queries::Register;

/// The error type if registering to the game server fails.
#[derive(Error, Debug)]
pub enum RegisterErr {
    /// The client must recreate a key-pair for this game server.
    /// This should usually never happen, because it would mean that
    /// the same public key is used between two different accounts.
    /// To prevent the case that a stolen private key for this
    /// account (on this game server) results in never being
    /// able to register, instead simply notify the client to
    /// create a new key-pair.
    #[error("The client has to recreate a key-pair for this game server.")]
    MustRecreateKeyPair,
    /// A http like error occurred.
    /// Client should retry later. (or the game server is configured incorrectly)
    #[error("{0}")]
    HttpLikeError(HttpLikeError),
    /// A database error happened.
    /// Client should retry later (also check if game server is configured correctly).
    #[error("{0}")]
    Database(anyhow::Error),
}

impl From<HttpLikeError> for RegisterErr {
    fn from(value: HttpLikeError) -> Self {
        Self::HttpLikeError(value)
    }
}

/// Props send by the client when trying to register
#[derive(Debug)]
pub struct ClientRegisterProps {
    /// The register token sent by the client
    pub register_token: RegisterToken,
}

/// Register a new user to the game server.
pub async fn register(
    io: &dyn Io,
    shared: Arc<Shared>,
    pool: &sqlx::MySqlPool,
    user_id: &UserId,
    client_props: ClientRegisterProps,
) -> anyhow::Result<bool, RegisterErr> {
    // now ask account server to verify the token
    let account_id = get_account_id_of_register_token(io, client_props.register_token).await?;

    let mut pool_con = pool
        .acquire()
        .await
        .map_err(|err| RegisterErr::Database(err.into()))?;
    let con = pool_con
        .acquire()
        .await
        .map_err(|err| RegisterErr::Database(err.into()))?;

    let qry = Register {
        user_id,
        account_id,
    };
    let qry_res = qry
        .query_mysql(&shared.mysql.register_statement)
        .execute(con)
        .await;

    let register_res = match qry_res {
        Ok(qry_res) => qry_res.rows_affected() >= 1,
        Err(err) => match err {
            sqlx::Error::Database(err) => match err.kind() {
                sqlx::error::ErrorKind::UniqueViolation => {
                    return Err(RegisterErr::MustRecreateKeyPair)
                }
                _ => {
                    return Err(err).map_err(|err| RegisterErr::Database(err.into()));
                }
            },
            _ => return Err(err).map_err(|err| RegisterErr::Database(err.into())),
        },
    };

    Ok(register_res)
}
