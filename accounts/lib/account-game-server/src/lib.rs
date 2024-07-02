//! This crate contains a base implementation for
//! a game server to do account related operations.
//! It helps sending data, storing results persistently.
//! This crate is not intended for creating UI,
//! any game logic, database implementations nor knowing about the communication details
//! (be it UDP, HTTP or other stuff).
//! It uses interfaces to abstract such concepts away.

#![deny(missing_docs)]
#![deny(warnings)]
#![deny(clippy::nursery)]
#![deny(clippy::all)]

/// Data types and operations related to
/// loggin in a user to the game server.
pub mod auto_login;
/// Data types used in the game server
/// for a mysql connection.
pub mod mysql;
/// Helpers to prepare the game server.
pub mod prepare;
/// Setup for databases and other stuff related to game servers.
pub mod setup;
/// Shared data that is used in the game
/// server implementation.
pub mod shared;
