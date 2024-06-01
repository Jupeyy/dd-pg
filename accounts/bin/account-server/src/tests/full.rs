use std::{str::FromStr, sync::Arc};

use account_client::{
    connect_game_server::connect_game_server, password_forgot::password_forgot,
    password_reset::password_reset,
};
use account_game_server::{auto_login::AutoLoginError, register::ClientRegisterProps};
use accounts_base::game_server::{
    self,
    server_id::game_server_group_id_from_pub_key,
    user_id::{user_id_from_pub_key, UserId},
};
use client_reqwest::client::ClientReqwestTokioFs;
use ed25519_dalek::ed25519::signature::SignerMut;
use email_address::EmailAddress;
use parking_lot::Mutex;

use crate::{
    tests::types::{TestAccServer, TestGameServer},
    verify_game_server_group::AdminAccountVerifyGameServerGroup,
};

#[tokio::test]
async fn account_full_process() {
    let test = async move {
        let secure_dir_client = tempfile::tempdir()?;
        let secure_dir_server = tempfile::tempdir()?;
        let http = reqwest::ClientBuilder::new().build()?;

        // account server setup
        let token: Arc<Mutex<String>> = Default::default();
        let reset_code: Arc<Mutex<String>> = Default::default();
        let acc_server = TestAccServer::new(token.clone(), reset_code.clone()).await?;
        let pool = acc_server.pool.clone();

        let client = ClientReqwestTokioFs::new(
            "http://localhost:4433".try_into()?,
            secure_dir_client.path(),
        )
        .await?;

        let register_data = account_client::register::register(
            EmailAddress::from_str("test@localhost")?,
            "MySup3rStrongPw!",
            &*client,
        )
        .await?;

        assert!(
            register_data.session_was_created.is_ok() || register_data.requires_verification,
            "{:?}",
            register_data.session_was_created
        );

        // auth fails because client is not verified yet
        let auth_res = account_client::auth::auth(&*client).await;
        assert!(auth_res.is_err());

        // verify client
        let token_url = token.lock().clone();
        http.get(token_url).send().await?;

        // now authing is ok
        let auth_res = account_client::auth::auth(&*client).await;
        assert!(auth_res.is_ok(), "{:?}", auth_res);

        // create a new session
        let login_data = account_client::login::login(
            EmailAddress::from_str("test@localhost")?,
            "MySup3rStrongPw!",
            &*client,
        )
        .await?;

        assert!(login_data.persist_res.is_ok());

        // authing is still ok (with the new session)
        let auth_res = account_client::auth::auth(&*client).await;
        assert!(auth_res.is_ok());
        let auth_res = auth_res?;

        // now comes game server
        let game_server = TestGameServer::new(&pool).await?;
        let game_server_data = game_server.game_server_data.clone();

        // prepared data
        let game_server_account_client = ClientReqwestTokioFs::new(
            "http://localhost:4433".try_into()?,
            secure_dir_server.path(),
        )
        .await?;

        // create an account for the game server
        let server_register = account_client::register::register(
            email_address::EmailAddress::from_str("testserver@localhost")?,
            "MySup3rStrongServerPw!",
            &*game_server_account_client,
        )
        .await?;
        assert!(
            server_register.requires_verification || server_register.session_was_created.is_ok()
        );

        // verify game server
        let token_url = token.lock().clone();
        http.get(token_url).send().await?;

        let server_auth_res = account_client::auth::auth(&*game_server_account_client).await?;

        crate::verify_game_server_group::admin_account_verify_game_server_group_impl(
            acc_server.shared.clone(),
            pool.clone(),
            AdminAccountVerifyGameServerGroup {
                admin_password: "test-admin-pw".into(),
                account_id: server_auth_res.account_id,
            },
        )
        .await?;

        let server_group_data = account_client::game_server_group_data::get_game_server_group_data(
            &server_auth_res.main_secret,
            &*game_server_account_client,
        )
        .await?;

        // This id must be verified and send to the client by either the game server
        // or something like a master server / CA.
        let game_server_group_id = game_server_group_id_from_pub_key(server_group_data.public_key);

        let mut client_on_server =
            connect_game_server(game_server_group_id, &auth_res.main_secret, &*client).await?;
        let client_public_key = client_on_server.public_key;

        // Note: this step is not required if client auth is used e.g. in a TLS auth
        {
            // client needs otp from game server
            // since the communication details don't matter, just fake the packets
            let game_server_otp = game_server::otp::generate_otp();

            // the client reads that packet
            let client_otp = game_server_otp;
            // signs the otp
            let client_signature = client_on_server.private_key.sign(&client_otp);
            // send otp, public key & signature to game server
            // game server verifies the otp & signature
            assert!(game_server::otp::verify_otp(
                game_server_otp,
                client_otp,
                &client_signature,
                &client_public_key
            ));
        }

        // Now the public key is also the unique identifier.
        // This id can directly be used, no further login required.
        let game_server_client_id = game_server::user_id::user_id_from_pub_key(client_public_key);

        // What the game server usually does is to provide a mechanism for the client
        // is to auto login the user, which then shows wether the user was already
        // registered, or if it should register. Usually the client tells the game server
        // if it is interested in registering (has an account).
        let auto_login_data = account_game_server::auto_login::auto_login(
            game_server_data.clone(),
            &pool,
            &game_server_client_id,
            true,
        )
        .await;
        assert!(auto_login_data.is_err_and(|err| matches!(err, AutoLoginError::MustRegister)));

        // user is not registered, so do it now
        // 1. user requests a token from the account server to allow
        //      the game server to verify the user's account_id.
        // 2. user sends this token and its account_id to game server and
        //      tells it that it wants to register.
        // 3. game server asks account server for account_id and verifies
        //      the correctness.

        // 1.
        let register_token =
            account_client::register_game_server::request_register_token_from_account_server(
                &*client,
            )
            .await?;

        // 2.
        let client_register_token = register_token;

        // 3.
        // actually register the user
        let register_res = account_game_server::register::register(
            &*game_server_account_client,
            game_server_data.clone(),
            &pool,
            &game_server_client_id,
            ClientRegisterProps {
                register_token: client_register_token,
            },
        )
        .await?;
        assert!(register_res);

        // user registered, the next auto login will also verify this
        let auto_login_data = account_game_server::auto_login::auto_login(
            game_server_data.clone(),
            &pool,
            &game_server_client_id,
            true,
        )
        .await?;
        assert!(auto_login_data.account_id.is_some());

        // now the user changes it's password (forget password)
        // it first requests this on the account server
        password_forgot(EmailAddress::from_str("test@localhost")?, &*client).await?;

        // now the password was changed, and we connect to the game server
        // again. The game server sees that the account id was already used
        // and thus requires a reregister (the reregister will use the previous
        // data).
        let reset_code = reset_code.lock().clone();
        let pw_reset_res = password_reset(
            EmailAddress::from_str("test@localhost")?,
            reset_code,
            "MyN3wStr@ngPw",
            &*client,
        )
        .await?;
        // already verified
        assert!(!pw_reset_res.requires_verification);

        // doing new auths should be no problem
        let auth_res = account_client::auth::auth(&*client).await;
        assert!(auth_res.is_ok());
        let auth_res = auth_res?;

        // ask again for game server connect
        let client_on_server =
            connect_game_server(game_server_group_id, &auth_res.main_secret, &*client).await?;

        // now the game server obviously doesn't know about this change
        let user_id: UserId = user_id_from_pub_key(client_on_server.public_key);
        let auto_login_data = account_game_server::auto_login::auto_login(
            game_server_data.clone(),
            &pool,
            &user_id,
            true,
        )
        .await;
        assert!(auto_login_data.is_err_and(|err| matches!(err, AutoLoginError::MustRegister)));

        // so the game server tells to re-register, which similar to accounts is not
        // deleting anything, just settings the ids right.
        let register_token =
            account_client::register_game_server::request_register_token_from_account_server(
                &*client,
            )
            .await?;
        // back to game server
        let reigster_res = account_game_server::register::register(
            &*game_server_account_client,
            game_server_data.clone(),
            &pool,
            &user_id,
            ClientRegisterProps { register_token },
        )
        .await?;
        assert!(reigster_res);

        // now the user should be registered
        let auto_login_data = account_game_server::auto_login::auto_login(
            game_server_data.clone(),
            &pool,
            &user_id,
            true,
        )
        .await?;
        assert!(auto_login_data.account_id.is_some());

        game_server.destroy().await?;
        // game server end

        acc_server.destroy().await?;

        anyhow::Ok(())
    };

    test.await.unwrap()
}
