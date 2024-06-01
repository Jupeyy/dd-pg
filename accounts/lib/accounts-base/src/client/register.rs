use serde::{Deserialize, Serialize};

use crate::{
    account_server::otp::Otp,
    client::{account_data::generate_account_data, session::generate_session_data},
};

/// All data required to create a new account
/// on the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterDataForServer {
    /// The email is used to reidentify the client.
    /// E.g. for password resets etc.
    pub email: email_address::EmailAddress,
    /// The account data that should be accosiated with the account
    pub account_data: crate::client::account_data::AccountDataForServer,
    /// Registering also establishes a session directly
    /// This is all data required
    pub session_data: crate::client::session::SessionDataForServer,
}

/// All data required to create a new account
/// on the client.
#[derive(Debug)]
pub struct RegisterDataForClient {
    /// The account that should be saved persistently on the client
    pub account_data: crate::client::account_data::AccountDataForClient,
    /// The session data that should be saved persistently on the client
    pub session_data: crate::client::session::SessionDataForClient,
}

/// All data required to create a new account
#[derive(Debug)]
pub struct RegisterData {
    /// All data for the account server
    pub for_server: RegisterDataForServer,
    /// All data that should be saved persistently
    pub for_client: RegisterDataForClient,
}

/// Collect all data required to register a new
/// account on the account server.
pub fn register_data(
    otps: [Otp; 2],
    hw_id: [u8; 32],
    email: email_address::EmailAddress,
    password: &str,
) -> anyhow::Result<RegisterData> {
    let [login_otp, auth_otp] = otps;
    let account_data = generate_account_data(&email, password)?;

    // Since creating a session is very similar to registering an account,
    // share some code here.
    let session_data = generate_session_data(login_otp, auth_otp, hw_id, email.clone(), password)?;

    Ok(RegisterData {
        for_server: RegisterDataForServer {
            email,
            account_data: account_data.for_server,
            session_data: session_data.for_server,
        },
        for_client: RegisterDataForClient {
            account_data: account_data.for_client,
            session_data: session_data.for_client,
        },
    })
}
