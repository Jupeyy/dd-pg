pub mod queries;

use std::sync::Arc;

use account_sql::query::Query;
use accounts_shared::{
    account_server::otp::generate_otp, client::account_token::AccountTokenEmailRequest,
};
use axum::{response, Json};
use base64::Engine;
use queries::AddAccountTokenEmail;
use sqlx::{Acquire, AnyPool};

use crate::{internal_err::InternalErr, shared::Shared};

pub async fn account_token_email(
    shared: Arc<Shared>,
    pool: AnyPool,
    Json(data): Json<AccountTokenEmailRequest>,
) -> response::Result<Json<()>> {
    account_token_email_impl(shared, pool, data)
        .await
        .map_err(|err| InternalErr(("account_token_email".into(), err)).into())
        .map(Json)
}

pub async fn account_token_email_impl(
    shared: Arc<Shared>,
    pool: AnyPool,
    data: AccountTokenEmailRequest,
) -> anyhow::Result<()> {
    // Add a account token and send it by email
    let token = generate_otp();
    let token_base_64 = base64::prelude::BASE64_URL_SAFE.encode(token);
    let query_add_account_token = AddAccountTokenEmail {
        token: &token,
        email: &data.email,
    };
    let mut connection = pool.acquire().await?;
    let con = connection.acquire().await?;

    let account_token_res = query_add_account_token
        .query(&shared.db.account_token_email_statement)
        .execute(&mut *con)
        .await?;
    anyhow::ensure!(
        account_token_res.rows_affected() >= 1,
        "No account token could be added."
    );

    shared
        .email
        .send_email(
            data.email.as_str(),
            "DDNet Account Token",
            format!(
                "Hello {},\nPlease use this token to verify your action \
                    :\n```\n{}\n```",
                data.email.local_part(),
                token_base_64
            ),
        )
        .await?;

    Ok(())
}
