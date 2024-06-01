use std::{str::FromStr, sync::Arc};

use account_client::auth::AuthResult;
use account_game_server::register::{ClientRegisterProps, RegisterErr};
use client_reqwest::client::ClientReqwestTokioFs;
use email_address::EmailAddress;
use parking_lot::Mutex;

use crate::tests::types::{TestAccServer, TestGameServer};

/// Tests related to a user registering on a game server.
#[tokio::test]
async fn user_register_hardening() {
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

        let auth_res = account_client::auth::auth(&*game_server_client).await;

        assert!(matches!(auth_res.unwrap_err(), AuthResult::FsLikeError(_)));

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

        // verify
        let token_url = token.lock().clone();
        http.get(token_url).send().await?;

        let auth_res = account_client::auth::auth(&*game_server_client)
            .await
            .unwrap();

        // invalid register token
        let register_res = account_game_server::register::register(
            &*game_server_client,
            game_server.game_server_data.clone(),
            &acc_server.pool,
            &Default::default(),
            ClientRegisterProps {
                register_token: Default::default(),
            },
        )
        .await;
        assert!(register_res.is_err_and(|err| matches!(err, RegisterErr::HttpLikeError(_))));

        // register the user
        let register_token = acc_server
            .shared
            .register_tokens
            .gen_register_token_for(auth_res.account_id);
        let register_res = account_game_server::register::register(
            &*game_server_client,
            game_server.game_server_data.clone(),
            &acc_server.pool,
            &Default::default(),
            ClientRegisterProps { register_token },
        )
        .await;
        assert!(register_res.is_ok());

        // now create a non acc user
        let user_id = [255; 32];
        let auto_login_res = account_game_server::auto_login::auto_login(
            game_server.game_server_data.clone(),
            &acc_server.pool,
            &user_id,
            false,
        )
        .await?;
        assert!(auto_login_res.account_id.is_none());

        // now this non acc user illegally registers with the first users account
        let register_token = acc_server
            .shared
            .register_tokens
            .gen_register_token_for(auth_res.account_id);
        let register_res = account_game_server::register::register(
            &*game_server_client,
            game_server.game_server_data.clone(),
            &acc_server.pool,
            &user_id,
            ClientRegisterProps { register_token },
        )
        .await;
        // so client would need to recreate the key pair in such case.
        assert!(register_res.is_err_and(|err| matches!(err, RegisterErr::MustRecreateKeyPair)));

        // quickly recreate the game server
        game_server.destroy().await?;
        let game_server = TestGameServer::new(&acc_server.pool).await?;

        // create a non acc user
        let user_id = [255; 32];
        let auto_login_res = account_game_server::auto_login::auto_login(
            game_server.game_server_data.clone(),
            &acc_server.pool,
            &user_id,
            false,
        )
        .await?;
        assert!(auto_login_res.account_id.is_none());

        // "upgrade" the non acc user to a user with account_id
        // This can only be done exactly once.
        let register_token = acc_server
            .shared
            .register_tokens
            .gen_register_token_for(auth_res.account_id);
        let register_res = account_game_server::register::register(
            &*game_server_client,
            game_server.game_server_data.clone(),
            &acc_server.pool,
            &user_id,
            ClientRegisterProps { register_token },
        )
        .await;
        assert!(register_res.is_ok());

        // create multiple non acc user
        let user_id = [244; 32];
        let auto_login_res = account_game_server::auto_login::auto_login(
            game_server.game_server_data.clone(),
            &acc_server.pool,
            &user_id,
            false,
        )
        .await?;
        assert!(auto_login_res.account_id.is_none());
        let user_id = [233; 32];
        let auto_login_res = account_game_server::auto_login::auto_login(
            game_server.game_server_data.clone(),
            &acc_server.pool,
            &user_id,
            false,
        )
        .await?;
        assert!(auto_login_res.account_id.is_none());
        let user_id = [222; 32];
        let auto_login_res = account_game_server::auto_login::auto_login(
            game_server.game_server_data.clone(),
            &acc_server.pool,
            &user_id,
            false,
        )
        .await?;
        assert!(auto_login_res.account_id.is_none());

        // and multiple users
        let register_data = account_client::register::register(
            EmailAddress::from_str("test2@localhost")?,
            "MySup3rStrong@Pw2",
            &*game_server_client,
        )
        .await;
        assert!(register_data.is_ok());

        let login_res = account_client::login::login(
            EmailAddress::from_str("test2@localhost")?,
            "MySup3rStrong@Pw2",
            &*game_server_client,
        )
        .await;
        assert!(login_res.is_ok());

        // verify
        let token_url = token.lock().clone();
        http.get(token_url).send().await?;

        let auth_res = account_client::auth::auth(&*game_server_client)
            .await
            .unwrap();

        let token = acc_server
            .shared
            .register_tokens
            .gen_register_token_for(auth_res.account_id);
        let register_res = account_game_server::register::register(
            &*game_server_client,
            game_server.game_server_data.clone(),
            &acc_server.pool,
            &user_id,
            ClientRegisterProps {
                register_token: token,
            },
        )
        .await;
        assert!(register_res.is_ok());

        game_server.destroy().await?;
        acc_server.destroy().await?;

        anyhow::Ok(())
    };
    test.await.unwrap();
}
