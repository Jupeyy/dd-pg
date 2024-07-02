use std::{str::FromStr, sync::Arc};

use account_client::{login::LoginResult, machine_id::machine_uid};
use accounts_base::{
    account_server::{
        login::{LoginResponse, LoginResponseSuccess},
        sign::AuthResponseSuccess,
    },
    client::session::generate_session_data,
};
use client_reqwest::client::ClientReqwestTokioFs;
use email_address::EmailAddress;
use parking_lot::Mutex;

use crate::tests::types::TestAccServer;

/// Tests related to [`LoginError`] & server side login
#[tokio::test]
async fn account_login_hardening() {
    let test = async move {
        let secure_dir_client = tempfile::tempdir()?;
        // account server setup
        let token: Arc<Mutex<String>> = Default::default();
        let reset_code: Arc<Mutex<String>> = Default::default();
        let acc_server = TestAccServer::new(token.clone(), reset_code.clone()).await?;

        let client = ClientReqwestTokioFs::new(
            "http://localhost:4433".try_into()?,
            secure_dir_client.path(),
        )
        .await?;

        let login_res = account_client::login::login(
            EmailAddress::from_str("test@localhost")?,
            "@nYPw32",
            &*client,
        )
        .await;

        assert!(matches!(
            login_res.unwrap_err(),
            LoginResult::InvalidPasswordOrEmail
        ));

        let register_data = account_client::register::register(
            EmailAddress::from_str("test@localhost")?,
            "MySup3rStrong@Pw",
            &*client,
        )
        .await;
        assert!(register_data.is_ok());

        let login_res = account_client::login::login(
            EmailAddress::from_str("test@localhost")?,
            "@nYPw32",
            &*client,
        )
        .await;

        assert!(matches!(
            login_res.unwrap_err(),
            LoginResult::InvalidPasswordOrEmail
        ));

        let login_res = account_client::login::login(
            EmailAddress::from_str("test2@localhost")?,
            "MySup3rStrong@Pw",
            &*client,
        )
        .await;

        assert!(matches!(
            login_res.unwrap_err(),
            LoginResult::InvalidPasswordOrEmail
        ));

        let hashed_hw_id = machine_uid()?;

        // invalid otps
        let session_data = generate_session_data(
            Default::default(),
            Default::default(),
            hashed_hw_id,
            EmailAddress::from_str("test@localhost")?,
            "MySup3rStrong@Pw",
        )?;
        let session_res = crate::session::create_session(
            acc_server.shared.clone(),
            acc_server.pool.clone(),
            session_data.for_server,
        )
        .await;
        assert!(session_res.is_err());

        // invalid auth otp
        let session_data = generate_session_data(
            acc_server.shared.otps.gen_otp(),
            Default::default(),
            hashed_hw_id,
            EmailAddress::from_str("test@localhost")?,
            "MySup3rStrong@Pw",
        )?;
        let session_res = crate::session::create_session(
            acc_server.shared.clone(),
            acc_server.pool.clone(),
            session_data.for_server,
        )
        .await;
        assert!(session_res.is_err());

        // valid otps
        let session_data = generate_session_data(
            acc_server.shared.otps.gen_otp(),
            acc_server.shared.otps.gen_otp(),
            hashed_hw_id,
            EmailAddress::from_str("test@localhost")?,
            "MySup3rStrong@Pw",
        )?;
        let session_res = crate::session::create_session(
            acc_server.shared.clone(),
            acc_server.pool.clone(),
            session_data.for_server,
        )
        .await;
        assert!(matches!(
            session_res.unwrap(),
            LoginResponse::Success(LoginResponseSuccess {
                auth: AuthResponseSuccess::NotVerified(_),
                main_secret: _
            })
        ));

        acc_server.destroy().await?;

        anyhow::Ok(())
    };
    test.await.unwrap();
}
