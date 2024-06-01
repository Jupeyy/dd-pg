use super::otp::{generate_otp, Otp};

/// Represents a reset code that the client can
/// sent to the account server to reset
/// a forgotten password.
pub type ResetCode = Otp;

/// Generates a new random reset code
pub fn generate_reset_code() -> ResetCode {
    generate_otp()
}
