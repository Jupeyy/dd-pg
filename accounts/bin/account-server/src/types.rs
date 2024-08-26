use serde::{Deserialize, Serialize};
use strum::{EnumString, IntoStaticStr};

// IMPORTANT: keep this in sync with the ty enum in src/setup/mysql/login_tokens.sql
/// The type of token that was created.
#[derive(Debug, Serialize, Deserialize, IntoStaticStr, EnumString, Clone, Copy)]
#[strum(serialize_all = "lowercase")]
pub enum TokenType {
    Email,
    Steam,
}
