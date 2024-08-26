/// All data that represents an account
/// for the client and account server.
/// This account is used to identify
/// uniquely on game-servers.
pub mod account_data;
/// A data type that is used for various account related operations.
pub mod account_token;
/// Data types and operations related to prepering
/// a delete requests.
pub mod delete;
/// Create hashes using [`argon2`].
pub mod hash;
/// Data types and operations related to prepering
/// a login request.
pub mod login;
/// Data types and operations related to getting
/// a token for email login.
pub mod login_token_email;
/// Data types and operations related to prepering
/// a logout requests.
pub mod logout;
/// Get a unique identifier per machine.
/// On unsupported systems this creates a default id.
pub mod machine_id;
/// Data types and operations that the client uses
/// when an auth to the account server is issued.
pub mod sign;
