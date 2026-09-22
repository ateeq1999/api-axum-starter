pub mod controller;
pub mod error;
pub mod processing;
pub mod service;
pub mod storage;

pub use controller::{own_router, public_router};
pub use error::AvatarError;
pub use service::AvatarService;
pub use storage::AvatarStorage;
