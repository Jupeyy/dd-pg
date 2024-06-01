pub mod queries;

use std::sync::Arc;

use account_sql::query::Query;
use accounts_base::{
    account_server::reset_code::generate_reset_code, client::password_forgot::PasswordForgotRequest,
};
use axum::{response, Json};
use base64::Engine;
use sqlx::{Acquire, MySqlPool};

use crate::{internal_err::InternalErr, shared::Shared};

use self::queries::{AddResetCode, EmailExistsCheck};

pub async fn password_forgot_request(
    shared: Arc<Shared>,
    pool: MySqlPool,
    Json(data): Json<PasswordForgotRequest>,
) -> response::Result<Json<()>> {
    password_forgot(shared, pool, data)
        .await
        .map_err(|err| InternalErr(("pw_forgot".into(), err)).into())
        .map(Json)
}

pub async fn password_forgot(
    shared: Arc<Shared>,
    pool: MySqlPool,
    data: PasswordForgotRequest,
) -> anyhow::Result<()> {
    let mut connection = pool.acquire().await?;
    let connection = connection.acquire().await?;

    // check if that email exists
    let email_exists = EmailExistsCheck { email: &data.email };

    let exists = email_exists
        .query_mysql(&shared.mysql.email_exists_statement)
        .fetch_optional(&mut *connection)
        .await?;

    // only actually send an email if that exists, else ignore
    if let Some(account_id) = exists {
        let account_id = EmailExistsCheck::row_data(&account_id)?;

        let reset_code = generate_reset_code();
        let add_reset_code = AddResetCode {
            account_id: account_id.account_id,
            reset_code: &reset_code,
        };

        let qry_res = add_reset_code
            .query_mysql(&shared.mysql.add_reset_code_statement)
            .execute(&mut *connection)
            .await?;

        anyhow::ensure!(
            qry_res.rows_affected() >= 1,
            "Reset code could not be created, this is a bug on the account server."
        );

        let reset_code_base64 = base64::prelude::BASE64_URL_SAFE.encode(reset_code);

        // result intentionally ignored
        let _ = shared
            .email
            .send_email(
                data.email.as_str(),
                "Password reset",
                format!(
                    "Hello {},\n\
                    To reset your password paste this reset code\n\
                    into the DDNet client's reset code field\n\
                    ```{}```
                    ",
                    data.email.local_part(),
                    reset_code_base64
                ),
            )
            .await;
    }

    Ok(())
}
