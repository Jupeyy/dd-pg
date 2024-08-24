use std::{str::FromStr, sync::Arc};

use account_client::sign::AuthResult;
use accounts_shared::{
    account_server::sign::AuthResponse,
    client::{session::generate_session_data, sign::prepare_auth_request},
};
use client_reqwest::client::ClientReqwestTokioFs;
use email_address::EmailAddress;
use parking_lot::Mutex;

use crate::tests::types::TestAccServer;

/// Tests related to [`AuthResult`] & server side auth
#[tokio::test]
async fn account_auth_hardening() {
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

        let auth_res = account_client::sign::auth(&*client).await;

        assert!(matches!(auth_res.unwrap_err(), AuthResult::FsLikeError(_)));

        let register_data = account_client::register::register(
            EmailAddress::from_str("test@localhost")?,
            "MySup3rStrong@Pw",
            &*client,
        )
        .await;
        assert!(register_data.is_ok());

        let login_res = account_client::login::login(
            EmailAddress::from_str("test@localhost")?,
            "MySup3rStrong@Pw",
            &*client,
        )
        .await;
        assert!(login_res.is_ok());

        let auth_res = account_client::sign::auth(&*client).await;
        assert!(matches!(
            auth_res.unwrap_err(),
            AuthResult::AccountNotVerified
        ));

        // make the session invalid
        acc_server.destroy().await?;
        let acc_server = TestAccServer::new(token.clone(), reset_code.clone()).await?;

        let auth_res = account_client::sign::auth(&*client).await;
        assert!(matches!(
            auth_res.unwrap_err(),
            AuthResult::SessionWasInvalid
        ));

        let auth_otp = acc_server.shared.otps.gen_otp();
        let mut session_data = generate_session_data(
            acc_server.shared.otps.gen_otp(),
            auth_otp,
            Default::default(),
            EmailAddress::from_str("test@localhost")?,
            "MySup3rStrong@Pw",
        )?;
        let auth_data = prepare_auth_request(
            Default::default(),
            Default::default(),
            &mut session_data.for_client.priv_key,
            session_data.for_client.pub_key,
        );
        let auth_res = crate::sign::auth(
            acc_server.shared.clone(),
            acc_server.pool.clone(),
            auth_data,
        )
        .await;
        assert!(auth_res.is_err());

        let auth_data = prepare_auth_request(
            auth_otp,
            Default::default(),
            &mut session_data.for_client.priv_key,
            session_data.for_client.pub_key,
        );
        let auth_res = crate::sign::auth(
            acc_server.shared.clone(),
            acc_server.pool.clone(),
            auth_data,
        )
        .await;
        assert!(matches!(auth_res.unwrap(), AuthResponse::Invalid));

        acc_server.destroy().await?;

        anyhow::Ok(())
    };
    test.await.unwrap();
}
