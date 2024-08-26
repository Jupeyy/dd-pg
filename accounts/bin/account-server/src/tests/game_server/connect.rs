use std::{str::FromStr, sync::Arc};

use account_client::{
    connect_game_server::ConnectGameServerError, password_forgot::password_forgot,
    password_reset::password_reset, sign::AuthResult,
};
use accounts_shared::game_server::server_id::game_server_group_id_from_pub_key;
use client_reqwest::client::ClientReqwestTokioFs;
use email_address::EmailAddress;
use parking_lot::Mutex;

use crate::{
    tests::types::{TestAccServer, TestGameServer},
    verify_game_server_group::AdminAccountVerifyGameServerGroup,
};

/// Tests related to a user registering on a game server.
/// And the game server connecting to the account server.
#[tokio::test]
async fn user_connect_hardening() {
    let test = async move {
        let secure_dir_gs_client = tempfile::tempdir()?;
        let http = reqwest::ClientBuilder::new().build()?;
        // account server setup
        let token: Arc<Mutex<String>> = Default::default();
        let reset_code: Arc<Mutex<String>> = Default::default();
        let acc_server = TestAccServer::new(token.clone(), reset_code.clone()).await?;
        let game_server = TestGameServer::new(&acc_server.pool).await?;

        let game_server_client = ClientReqwestTokioFs::new(
            "http://localhost:4433".try_into()?,
            secure_dir_gs_client.path(),
        )
        .await?;

        let auth_res = account_client::sign::auth(&*game_server_client).await;

        assert!(matches!(auth_res.unwrap_err(), AuthResult::FsLikeError(_)));

        // not logged in.
        let connect_res = account_client::connect_game_server::connect_game_server(
            Default::default(),
            &[],
            &*game_server_client,
        )
        .await;
        assert!(connect_res.is_err_and(|err| matches!(err, ConnectGameServerError::AuthInvalid)));

        let group_data = account_client::game_server_group_data::get_game_server_group_data(
            &[],
            &*game_server_client,
        )
        .await;
        assert!(group_data.is_err_and(|err| matches!(err, ConnectGameServerError::AuthInvalid)));

        let register_data = account_client::register::register(
            EmailAddress::from_str("test@localhost")?,
            "MySup3rStrong@Pw",
            &*game_server_client,
        )
        .await;
        assert!(register_data.is_ok());

        let login_res = account_client::login::login(
            EmailAddress::from_str("test@localhost")?,
            "MySup3rStrong@Pw",
            &*game_server_client,
        )
        .await;
        assert!(login_res.is_ok());

        let auth_offline =
            account_client::login::login_offline("MySup3rStrong@Pw", &*game_server_client).await?;

        // not verified yet (account created and session created.)
        let connect_res = account_client::connect_game_server::connect_game_server(
            Default::default(),
            &auth_offline.main_secret,
            &*game_server_client,
        )
        .await;
        assert!(connect_res.is_err_and(|err| matches!(err, ConnectGameServerError::AuthInvalid)));
        let group_data = account_client::game_server_group_data::get_game_server_group_data(
            &auth_offline.main_secret,
            &*game_server_client,
        )
        .await;
        assert!(group_data.is_err_and(|err| matches!(err, ConnectGameServerError::AuthInvalid)));

        // verify
        let token_url = token.lock().clone();
        http.get(token_url).send().await?;

        let auth_res = account_client::sign::auth(&*game_server_client).await?;

        let group_data = account_client::game_server_group_data::get_game_server_group_data(
            &auth_res.main_secret,
            &*game_server_client,
        )
        .await;
        assert!(group_data.is_err_and(|err| matches!(err, ConnectGameServerError::AuthInvalid)));

        crate::verify_game_server_group::admin_account_verify_game_server_group_impl(
            acc_server.shared.clone(),
            acc_server.pool.clone(),
            AdminAccountVerifyGameServerGroup {
                admin_password: "test-admin-pw".into(),
                account_id: auth_res.account_id,
            },
        )
        .await?;

        let group_data = account_client::game_server_group_data::get_game_server_group_data(
            &auth_res.main_secret,
            &*game_server_client,
        )
        .await?;
        let group_id = game_server_group_id_from_pub_key(group_data.public_key);

        // reset the account which should invalidate the above game server group data
        password_forgot(
            EmailAddress::from_str("test@localhost")?,
            &*game_server_client,
        )
        .await?;
        let reset_code = reset_code.lock().clone();
        password_reset(
            EmailAddress::from_str("test@localhost")?,
            reset_code,
            "MySup3rStrong@Pw",
            &*game_server_client,
        )
        .await?;

        let auth_res = account_client::sign::auth(&*game_server_client).await?;

        let group_data = account_client::game_server_group_data::get_game_server_group_data(
            &auth_res.main_secret,
            &*game_server_client,
        )
        .await?;
        let group_id2 = game_server_group_id_from_pub_key(group_data.public_key);
        assert!(group_id != group_id2);

        let connect_res = account_client::connect_game_server::connect_game_server(
            group_id,
            &auth_res.main_secret,
            &*game_server_client,
        )
        .await;
        assert!(connect_res
            .is_err_and(|err| matches!(err, ConnectGameServerError::NoSuchGameServerGroup)));

        let connect_res = account_client::connect_game_server::connect_game_server(
            group_id,
            &[],
            &*game_server_client,
        )
        .await;
        assert!(connect_res.is_err_and(|err| matches!(err, ConnectGameServerError::CryptFailed)));

        game_server.destroy().await?;
        acc_server.destroy().await?;

        anyhow::Ok(())
    };
    test.await.unwrap();
}
