use std::{str::FromStr, sync::Arc};

use account_client::register::RegisterError;
use accounts_base::account_server::{auth::AuthResponse, register::RegisterResponse};
use anyhow::anyhow;
use client_reqwest::client::ClientReqwestTokioFs;
use email_address::EmailAddress;
use parking_lot::Mutex;

use crate::tests::types::TestAccServer;

/// Tests related to [`RegisterError`] & server side register
#[tokio::test]
async fn account_register_hardening() {
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

        let register_data = account_client::register::register(
            EmailAddress::from_str("test@localhost")?,
            "myweakpw",
            &*client,
        )
        .await;

        assert!(matches!(
            register_data.unwrap_err(),
            RegisterError::PasswordTooWeak
        ));

        let client = ClientReqwestTokioFs::new(
            "http://localhost:4334".try_into()?, // NOTE: wrong port here
            secure_dir_client.path(),
        )
        .await?;

        let register_data = account_client::register::register(
            EmailAddress::from_str("test@localhost")?,
            "MySup3rStrong@Pw",
            &*client,
        )
        .await;

        assert!(matches!(
            register_data.unwrap_err(),
            RegisterError::HttpLikeError(_)
        ));

        // invalid otps
        let register_data = accounts_base::client::register::register_data(
            Default::default(),
            Default::default(),
            EmailAddress::from_str("test@localhost")?,
            "",
        )?;
        let register_res = crate::register::register_impl(
            acc_server.shared.clone(),
            acc_server.pool.clone(),
            register_data.for_server,
        )
        .await;
        let RegisterResponse::Success { auth_response, .. } = register_res.unwrap().0 else {
            return Err(anyhow!("not success"));
        };
        assert!(matches!(auth_response, AuthResponse::Invalid));

        acc_server.destroy().await?;
        let acc_server = TestAccServer::new(token.clone(), reset_code.clone()).await?;

        let mut register_data = accounts_base::client::register::register_data(
            [
                acc_server.shared.otps.gen_otp(),
                acc_server.shared.otps.gen_otp(),
            ],
            Default::default(),
            EmailAddress::from_str("test@localhost")?,
            "",
        )?;
        // invalid email
        register_data.for_server.session_data.email = EmailAddress::from_str("test2@localhost")?;
        let register_res = crate::register::register_impl(
            acc_server.shared.clone(),
            acc_server.pool.clone(),
            register_data.for_server,
        )
        .await;
        let RegisterResponse::Success { auth_response, .. } = register_res.unwrap().0 else {
            return Err(anyhow!("not success"));
        };
        assert!(matches!(auth_response, AuthResponse::Invalid));

        acc_server.destroy().await?;

        anyhow::Ok(())
    };
    let test_duplicate_register = async move {
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

        let register_data = account_client::register::register(
            EmailAddress::from_str("test@localhost")?,
            "mYSuperStr0ngPw@!",
            &*client,
        )
        .await;

        assert!(register_data.is_ok());

        let register_data = account_client::register::register(
            EmailAddress::from_str("test@localhost")?,
            "mYSuperStr0ngPw@!",
            &*client,
        )
        .await;
        assert!(register_data
            .is_err_and(|err| matches!(err, RegisterError::AccountWithEmailAlreadyExists)));

        acc_server.destroy().await?;

        anyhow::Ok(())
    };
    test.await.unwrap();
    test_duplicate_register.await.unwrap();
}
