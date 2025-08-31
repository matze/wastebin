#![cfg_attr(test, allow(clippy::map_unwrap_or, clippy::unwrap_used))]

//! Core crate for code shared between the server and the control binaries.

pub mod crypto;
pub mod db;
pub mod env;
pub mod expiration;
pub mod id;
