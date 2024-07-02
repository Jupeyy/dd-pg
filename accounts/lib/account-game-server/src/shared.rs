use crate::mysql::MySqlConnectionShared;
/// Various data that is shared for the async
/// implementations
pub struct Shared {
    /// Prepared mysql statements
    pub mysql: MySqlConnectionShared,
}
