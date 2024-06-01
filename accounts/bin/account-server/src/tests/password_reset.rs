use std::{str::FromStr, sync::Arc};

use accounts_base::client::password_reset::PasswordResetRequest;
use email_address::EmailAddress;
use parking_lot::Mutex;

use crate::tests::types::TestAccServer;

/// Tests related to server side password reset
#[tokio::test]
async fn account_password_reset_hardening() {
    let test = async move {
        // account server setup
        let token: Arc<Mutex<String>> = Default::default();
        let reset_code: Arc<Mutex<String>> = Default::default();
        let acc_server = TestAccServer::new(token.clone(), reset_code.clone()).await?;

        let register_data = accounts_base::client::register::register_data(
            [
                acc_server.shared.otps.gen_otp(),
                acc_server.shared.otps.gen_otp(),
            ],
            Default::default(),
            EmailAddress::from_str("test@localhost")?,
            "",
        )?;
        let reset_res = crate::password_reset::password_reset(
            acc_server.shared.clone(),
            acc_server.pool.clone(),
            PasswordResetRequest {
                register_data: register_data.for_server,
                reset_code_base64: "".into(),
            },
        )
        .await;
        assert!(reset_res.is_err());

        acc_server.destroy().await?;

        anyhow::Ok(())
    };
    test.await.unwrap();
}
