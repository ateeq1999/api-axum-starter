//! User accounts: administration, profiles, and the account operations other features use.
//!
//! - `controllers/`   one file per endpoint
//! - `services/`      one file per use case, all on `UsersService`
//! - `repositories/`  one file per responsibility, all on `UsersRepository`
//! - `dto/`           one file per request or response shape

pub mod controllers;
pub mod dto;
pub mod entity;
pub mod error;
pub mod policy;
pub mod repositories;
pub mod services;

pub use controllers::router;
pub use entity::User;
pub use error::UsersError;
pub use repositories::UsersRepository;
pub use services::{UsersService, normalize_email};
