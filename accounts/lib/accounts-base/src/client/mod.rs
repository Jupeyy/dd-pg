/// All data that represents an account
/// for the client and server.
/// This account is used to identify
/// uniquely on game-servers.
pub mod account_data;
/// Data types and operations that the client uses
/// when an auth to the account server is issued.
pub mod auth;
/// Data types and operations required to create a
/// data for a game server group.
pub mod game_server_data;
/// Data types and operations required to request
/// one time passwords from the account server.
pub mod otp;
/// Password operations on and for the client .
pub mod password;
/// Data types and operations required to do a
/// password forgot process.
pub mod password_forgot;
/// Data types and operations related to resetting a password
/// on the account server.
pub mod password_reset;
/// Data types and operations requiered for registering to
/// the account server.
pub mod register;
/// Data types required to request a register token
/// from the account server.
pub mod reigster_token;
/// Data types and operations required to create a session
/// on the account server.
pub mod session;
