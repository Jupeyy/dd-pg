use std::{str::FromStr, sync::Arc};

use account_client::{login::LoginResult, login_token_email::LoginTokenResult};
use accounts_shared::account_server::{errors::AccountServerRequestError, login::LoginError};
use client_reqwest::client::ClientReqwestTokioFs;
use email_address::EmailAddress;
use parking_lot::Mutex;

use crate::tests::types::TestAccServer;

/// Tests related to [`LoginTokenResult`] & [`LoginResult`] & server side login
#[tokio::test]
async fn login_rate_limit() {
    let test = async move {
        let secure_dir_client = tempfile::tempdir()?;
        // account server setup
        let token: Arc<Mutex<String>> = Default::default();
        let reset_code: Arc<Mutex<String>> = Default::default();
        let acc_server = TestAccServer::new(token.clone(), reset_code.clone(), true).await?;

        let client = ClientReqwestTokioFs::new(
            "http://localhost:4433".try_into()?,
            secure_dir_client.path(),
        )
        .await?;

        account_client::login_token_email::login_token_email(
            EmailAddress::from_str("test@localhost")?,
            &*client,
        )
        .await?;

        // do actual login for client
        let token_b64 = token.lock().clone();
        let _account_data = account_client::login::login(token_b64.clone(), &*client).await?;

        let err = account_client::login_token_email::login_token_email(
            EmailAddress::from_str("test@localhost")?,
            &*client,
        )
        .await
        .unwrap_err();
        assert!(matches!(
            err,
            LoginTokenResult::AccountServerRequstError(AccountServerRequestError::RateLimited(_))
        ));

        let err = account_client::login::login(token_b64.clone(), &*client)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            LoginResult::AccountServerRequstError(AccountServerRequestError::RateLimited(_))
        ));

        acc_server.destroy().await?;

        anyhow::Ok(())
    };
    test.await.unwrap();
}

#[tokio::test]
async fn login_hardening() {
    let test = async move {
        let secure_dir_client = tempfile::tempdir()?;
        // account server setup
        let token: Arc<Mutex<String>> = Default::default();
        let reset_code: Arc<Mutex<String>> = Default::default();
        let acc_server = TestAccServer::new(token.clone(), reset_code.clone(), false).await?;

        let client = ClientReqwestTokioFs::new(
            "http://localhost:4433".try_into()?,
            secure_dir_client.path(),
        )
        .await?;

        account_client::login_token_email::login_token_email(
            EmailAddress::from_str("test@localhost")?,
            &*client,
        )
        .await?;

        let token_b64 = token.lock().clone();
        // already use the token
        let _account_data = account_client::login::login(token_b64.clone(), &*client).await?;

        let err = account_client::login::login("invalid".to_string(), &*client)
            .await
            .unwrap_err();
        assert!(matches!(err, LoginResult::Other(_)));

        // token can't be valid at this point anymore
        let err = account_client::login::login(token_b64, &*client)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            LoginResult::AccountServerRequstError(AccountServerRequestError::LogicError(
                LoginError::TokenInvalid
            ))
        ));

        acc_server.destroy().await?;

        anyhow::Ok(())
    };
    test.await.unwrap();
}
