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

/// Auth for an already existing session
pub mod auth;
/// Data types and operations required to generate
/// key-pairs for a game server group and store those
/// on the account server.
pub mod connect_game_server;
/// Types related to errors during client operations.
pub mod errors;
/// Data types and operations related to a game server
/// getting server group data.
pub mod game_server_group_data;
/// Communication interface for the client to
/// do requests to the account server.
pub mod interface;
/// Creates a new session.
pub mod login;
/// Get a unique identifier per machine.
/// On unsupported systems this creates a default id.
pub mod machine_id;
/// Trigger a password reset process.
pub mod password_forgot;
/// Data types and operations required to reset
/// a password on the account server.
pub mod password_reset;
/// Full registering process for an account on
/// the account server.
pub mod register;
/// Data types and operations required to register
/// an account on the game server.
pub mod register_game_server;
/// Data types and operations related to register tokens.
pub mod register_token;
