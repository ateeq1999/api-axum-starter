pub mod controllers;
pub mod error;
pub mod processing;
pub mod services;
pub mod storage;

pub use controllers::{own_router, public_router};
pub use error::AvatarError;
pub use services::AvatarService;
pub use storage::AvatarStorage;
