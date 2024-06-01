use ed25519_dalek::{Signature, VerifyingKey};

pub use crate::account_server::otp::Otp;

/// Generates an one time password
/// that the game server should send to the client
/// as soon as possible.
/// This one time password should then be signed by the client
/// using its account's public key.
/// The game server can then verify that the otp was
/// signed by the client which ultimately allows
/// the game server know that however owns that
/// public key can also be identified by exactly that public
/// key. So the public key here is also an unique
/// identifier.  
/// Note: If the connection uses a client auth (e.g. using TLS), these
/// functions can be safely ignored.
pub fn generate_otp() -> Otp {
    crate::account_server::otp::generate_otp()
}

/// Verifies that the otp the client send back to the server is:
/// - Equal to the one stored on the game server
/// - The public key verifies the otp signature of the client's otp
///
/// Note: If the connection uses a client auth (e.g. using TLS), these
/// functions can be safely ignored.
pub fn verify_otp(
    game_server_otp: Otp,
    client_otp: Otp,
    client_otp_signature: &Signature,
    client_public_key: &VerifyingKey,
) -> bool {
    game_server_otp == client_otp
        && client_public_key
            .verify_strict(&client_otp, client_otp_signature)
            .is_ok()
}
