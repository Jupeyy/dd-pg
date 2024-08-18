use std::{sync::Arc, time::Duration};

use account_sql::query::Query;
use queries::{CleanupAccountTokens, CleanupCerts, CleanupLoginTokens};
use sqlx::{Acquire, AnyPool, Executor};

use crate::shared::Shared;

pub mod queries;

pub async fn update_impl(pool: &AnyPool, shared: &Arc<Shared>) {
    if let Ok(mut connection) = pool.acquire().await {
        if let Ok(connection) = connection.acquire().await {
            // cleanup login tokens
            let _ = connection
                .execute(CleanupLoginTokens {}.query(&shared.db.cleanup_login_tokens_statement))
                .await;

            // cleanup account tokens
            let _ = connection
                .execute(CleanupAccountTokens {}.query(&shared.db.cleanup_account_tokens_statement))
                .await;

            // cleanup certs
            let _ = connection
                .execute(CleanupCerts {}.query(&shared.db.cleanup_certs_statement))
                .await;
        }
    }
}

pub async fn update(pool: AnyPool, shared: Arc<Shared>) -> ! {
    loop {
        // only do the update once per hour
        tokio::time::sleep(Duration::from_secs(60 * 60 * 24)).await;

        update_impl(&pool, &shared).await;
    }
}
