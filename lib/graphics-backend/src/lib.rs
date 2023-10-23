//#![deny(missing_docs)]
//#![deny(warnings)]
//#![deny(clippy::nursery)]
//#![deny(clippy::pedantic)]
//#![deny(clippy::all)]

#![allow(clippy::all)]
#![warn(clippy::char_lit_as_u8)]
#![warn(clippy::needless_return)]
#![warn(clippy::double_must_use)]
#![warn(clippy::needless_pass_by_ref_mut)]
#![warn(clippy::needless_lifetimes)]
#![warn(clippy::unnecessary_cast)]
#![warn(clippy::deref_addrof)]
#![warn(clippy::unnecessary_mut_passed)]
#![warn(clippy::unnecessary_unwrap)]
// allowed
#![allow(clippy::eq_op)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::redundant_pattern_matching)]
// temporary
#![allow(clippy::field_reassign_with_default)]

pub mod backend;
mod backend_mt;
mod backends;
pub mod checker;
pub mod types;
pub mod window;
