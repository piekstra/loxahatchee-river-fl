//! Authentication: the keychain-backed login [`session`](self::session) and the
//! [`Secret`] wrapper it stores credentials in.

pub mod secrets;
pub mod session;

pub use secrets::{CredentialStore, Secret};
pub use session::Session;
