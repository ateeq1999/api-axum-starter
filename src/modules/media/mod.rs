//! User-uploaded media (images, documents, short audio/video) stored through the shared
//! `infra::storage::ObjectStorage` (local disk or S3). Every file is private to its owner (and
//! admins): read it back through `GET /api/v1/media/{id}/content` with the same credentials.

pub mod controller;
pub mod dto;
pub mod entity;
pub mod error;
pub mod repository;
pub mod service;

pub use controller::router;
pub use error::MediaError;
pub use repository::MediaRepository;
pub use service::MediaService;
