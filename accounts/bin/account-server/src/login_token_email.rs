pub mod queries;

use std::sync::Arc;

use account_sql::query::Query;
use accounts_shared::{
    account_server::otp::generate_otp, client::login_token_email::LoginTokenEmailRequest,
};
use axum::{response, Json};
use base64::Engine;
use sqlx::{Acquire, AnyPool};

use crate::{internal_err::InternalErr, login_token_email::queries::AddLoginToken, shared::Shared};

pub async fn login_token_email(
    shared: Arc<Shared>,
    pool: AnyPool,
    Json(data): Json<LoginTokenEmailRequest>,
) -> response::Result<Json<()>> {
    login_token_email_impl(shared, pool, data)
        .await
        .map_err(|err| InternalErr(("login_token_email".into(), err)).into())
        .map(Json)
}

pub async fn login_token_email_impl(
    shared: Arc<Shared>,
    pool: AnyPool,
    data: LoginTokenEmailRequest,
) -> anyhow::Result<()> {
    // write the new account to the database
    // Add a login token and send it by email
    let token = generate_otp();
    let token_base_64 = base64::prelude::BASE64_URL_SAFE.encode(token);
    let query_add_login_token = AddLoginToken {
        token: &token,
        email: &data.email,
    };
    let mut connection = pool.acquire().await?;
    let con = connection.acquire().await?;

    let login_token_res = query_add_login_token
        .query(&shared.db.login_token_email_statement)
        .execute(&mut *con)
        .await?;
    anyhow::ensure!(
        login_token_res.rows_affected() >= 1,
        "No login token could be added."
    );

    shared
        .email
        .send_email(
            data.email.as_str(),
            "DDNet Account Login",
            format!(
                "Hello {},\nTo finish the login into your account \
                    please use the following code:\n```\n{}\n```",
                data.email.local_part(),
                token_base_64
            ),
        )
        .await?;

    Ok(())
}
