use std::{str::FromStr, sync::Arc};

use accounts_base::{account_server::cert_account_ext::AccountCertExt, game_server};
use anyhow::anyhow;
use client_reqwest::client::ClientReqwestTokioFs;
use email_address::EmailAddress;
use parking_lot::Mutex;
use x509_cert::der::Decode;

use crate::tests::types::{TestAccServer, TestGameServer};

#[tokio::test]
async fn account_full_process() {
    let test = async move {
        let secure_dir_client = tempfile::tempdir()?;

        // account server setup
        let token: Arc<Mutex<String>> = Default::default();
        let reset_code: Arc<Mutex<String>> = Default::default();
        let acc_server = TestAccServer::new(token.clone(), reset_code.clone()).await?;
        let pool = acc_server.pool.clone();
        let _shared = acc_server.shared.clone();

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
        let _account_data = account_client::login::login(token_b64, &*client).await?;

        // create a current signed certificate on the account server
        let cert = account_client::sign::sign(&*client).await?;

        let Ok(Some((_, account_data))) = x509_cert::Certificate::from_der(&cert.certificate_der)?
            .tbs_certificate
            .get::<AccountCertExt>()
        else {
            return Err(anyhow!("no valid account data found."));
        };

        assert!(account_data.data.account_id >= 1);

        // now comes game server
        let game_server = TestGameServer::new(&pool).await?;
        let game_server_data = game_server.game_server_data.clone();

        // Now use the client cert to get the user id, which is either the account id
        // or the public key fingerprint
        let pub_key_account_server = acc_server.shared.signing_key.verifying_key();
        let user_id = game_server::user_id::user_id_from_pub_key(
            pub_key_account_server,
            cert.certificate_der,
        );
        assert!(user_id.account_id.is_some());

        // What the game server usually does is to provide a mechanism for the client
        // to auto login the user, this automatically registers new users.
        // And in case of an "upgrade" so that a user previously had no account id but
        // uses the same public key again, it will move the points of this public key
        // to that account.
        let auto_login_res =
            account_game_server::auto_login::auto_login(game_server_data.clone(), &pool, &user_id)
                .await;
        assert!(auto_login_res.is_ok());
        /*
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
                let auth_res = account_client::sign::auth(&*client).await;
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

        */
        game_server.destroy().await?;
        // game server end

        acc_server.destroy().await?;

        anyhow::Ok(())
    };

    test.await.unwrap()
}
