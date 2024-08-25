//! This crate contains a base implementation for
//! a client to do account related operations.
//! It helps sending data, storing results persistently.
//! This crate is not intended for creating UI,
//! any game logic nor knowing about the communication details
//! (be it UDP, HTTP or other stuff).
//! It uses interfaces to abstract such concepts away.

#![deny(missing_docs)]
#![deny(warnings)]
#![deny(clippy::nursery)]
#![deny(clippy::all)]

pub(crate) mod safe_interface;

/// Requests an account token email based.
pub mod account_token_email;
/// Operations related to getting the account server certificates
pub mod certs;
/// Requests to delete sessions of an account or the account
/// itself.
pub mod delete;
/// Types related to errors during client operations.
pub mod errors;
/// Communication interface for the client to
/// do requests to the account server.
pub mod interface;
/// Requests to create a new login for the corresponding
/// account.
pub mod login;
/// Requests a token for an email based login.
pub mod login_token_email;
/// Request to log out the current user session.
pub mod logout;
/// Sign an already existing session key-pair
/// with a certificate on the account server.
pub mod sign;
