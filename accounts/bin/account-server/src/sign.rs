pub mod queries;

use std::{str::FromStr, sync::Arc, time::Duration};

use account_sql::query::Query;
use accounts_base::{
    account_server::{
        cert_account_ext::{AccountCertData, AccountCertExt},
        sign::SignResponse,
    },
    client::sign::SignRequest,
};
use axum::{response, Json};
use p256::ecdsa::DerSignature;
use sqlx::{Acquire, MySqlPool};
use x509_cert::builder::Builder;
use x509_cert::der::Encode;
use x509_cert::{
    builder::Profile, name::Name, serial_number::SerialNumber, spki::SubjectPublicKeyInfoOwned,
    time::Validity,
};

use crate::{internal_err::InternalErr, shared::Shared};

use self::queries::AuthAttempt;

pub async fn sign_request(
    shared: Arc<Shared>,
    pool: MySqlPool,
    Json(data): Json<SignRequest>,
) -> response::Result<Json<SignResponse>> {
    sign(shared, pool, data)
        .await
        .map_err(|err| InternalErr(("sign".into(), err)).into())
        .map(Json)
}

pub async fn sign(
    shared: Arc<Shared>,
    pool: MySqlPool,
    data: SignRequest,
) -> anyhow::Result<SignResponse> {
    let mut connection = pool.acquire().await?;
    let connection = connection.acquire().await?;

    let qry = AuthAttempt { data: &data };
    let row = qry
        .query_mysql(&shared.mysql.auth_attempt_statement)
        .fetch_one(connection)
        .await?;
    let auth_data = AuthAttempt::row_data(&row)?;

    let serial_number = SerialNumber::from(42u32);
    let validity = Validity::from_now(Duration::new(60 * 60, 0))?;
    let profile = Profile::Root;
    let subject = Name::from_str("CN=World domination corporation,O=World domination Inc,C=US")?;

    let pub_key = SubjectPublicKeyInfoOwned::from_key(data.pub_key)?;

    let mut builder = x509_cert::builder::CertificateBuilder::new(
        profile,
        serial_number,
        validity,
        subject,
        pub_key,
        &shared.signing_key,
    )?;
    let unix_utc = auth_data
        .creation_date
        .signed_duration_since(sqlx::types::chrono::DateTime::UNIX_EPOCH);

    builder.add_extension(&AccountCertExt {
        data: AccountCertData {
            account_id: auth_data.account_id,
            utc_time_since_unix_epoch_millis: unix_utc.num_milliseconds(),
        },
    })?;
    let cert = builder.build::<DerSignature>()?.to_der()?;

    Ok(SignResponse::Success { cert_der: cert })
}
