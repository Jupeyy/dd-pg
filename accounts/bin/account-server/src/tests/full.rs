use std::{
    str::FromStr,
    sync::Arc,
    time::{Duration, SystemTime},
};

use account_client::{
    certs::{certs_to_pub_keys, download_certs},
    logout::logout,
    sign::SignResult,
};
use accounts_shared::{account_server::cert_account_ext::AccountCertExt, game_server};
use anyhow::anyhow;
use client_reqwest::client::ClientReqwestTokioFs;
use email_address::EmailAddress;
use parking_lot::Mutex;
use x509_cert::der::Decode;

use crate::{
    certs::PrivateKeys,
    generate_new_signing_keys,
    tests::types::{TestAccServer, TestGameServer},
    update::update_impl,
};

#[tokio::test]
async fn account_full_process() {
    let test = async move {
        let secure_dir_client = tempfile::tempdir()?;

        // account server setup
        let token: Arc<Mutex<String>> = Default::default();
        let account_token: Arc<Mutex<String>> = Default::default();
        let acc_server = TestAccServer::new(token.clone(), account_token.clone(), false).await?;
        let pool = acc_server.pool.clone();
        let shared = acc_server.shared.clone();

        let url = "http://localhost:4433";
        let client = ClientReqwestTokioFs::new(url.try_into()?, secure_dir_client.path()).await?;

        let login = || {
            Box::pin(async {
                account_client::login_token_email::login_token_email(
                    EmailAddress::from_str("test@localhost")?,
                    &*client,
                )
                .await?;

                // do actual login for client
                let token_b64 = token.lock().clone();
                let _account_data = account_client::login::login(token_b64, &*client).await?;
                anyhow::Ok(())
            })
        };
        // the first login will also create the account
        login().await?;

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

        // emulate a game server that downloads certs from account server to validate
        // the account cert from the client.
        let certs = download_certs(&*client).await?;
        let keys = certs_to_pub_keys(&certs);

        // Now use the client cert to get the user id, which is either the account id
        // or the public key fingerprint
        let user_id = game_server::user_id::user_id_from_cert(&keys, cert.certificate_der);
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

        // remove this session
        logout(&*client).await?;

        // signing should fali now
        assert!(matches!(
            account_client::sign::sign(&*client).await,
            Err(SignResult::FsLikeError(_))
        ));

        // login again
        login().await?;

        // remove all sessions
        account_client::account_token_email::account_token_email(
            EmailAddress::from_str("test@localhost")?,
            &*client,
        )
        .await?;
        let account_token_b64 = account_token.lock().clone();
        account_client::delete::delete_sessions(account_token_b64, &*client).await?;

        // signing should fali now
        assert!(matches!(
            account_client::sign::sign(&*client).await,
            Err(SignResult::FsLikeError(_))
        ));

        // login again
        login().await?;

        // delete account
        account_client::account_token_email::account_token_email(
            EmailAddress::from_str("test@localhost")?,
            &*client,
        )
        .await?;
        let account_token_b64 = account_token.lock().clone();
        account_client::delete::delete(account_token_b64, &*client).await?;

        // signing should fali now
        assert!(matches!(
            account_client::sign::sign(&*client).await,
            Err(SignResult::FsLikeError(_))
        ));

        game_server.destroy().await?;
        // game server end

        // test some account server related stuff
        // updates (which usually do cleanup tasks)
        update_impl(&pool, &shared).await;

        // generate new signing keys
        let cur_keys = shared.signing_keys.read().clone();
        let mut fake_cert = cur_keys.current_cert.clone();
        fake_cert.tbs_certificate.validity.not_after = SystemTime::now().try_into().unwrap();
        let fake_keys = PrivateKeys {
            current_key: cur_keys.current_key.clone(),
            current_cert: fake_cert,
            next_key: cur_keys.next_key.clone(),
            next_cert: cur_keys.next_cert.clone(),
        };
        *shared.signing_keys.write() = Arc::new(fake_keys);
        generate_new_signing_keys(&pool, &shared).await;

        // if above worked both keys should be around same lifetime
        let cur_keys = shared.signing_keys.read().clone();
        // assumes that this test does never run for a whole day...
        anyhow::ensure!(
            cur_keys
                .current_cert
                .tbs_certificate
                .validity
                .not_after
                .to_system_time()
                + Duration::from_secs(60 * 60 * 24)
                > cur_keys
                    .next_cert
                    .tbs_certificate
                    .validity
                    .not_after
                    .to_system_time(),
            "certs do not have a similar lifetime"
        );

        acc_server.destroy().await?;

        anyhow::Ok(())
    };

    test.await.unwrap()
}
