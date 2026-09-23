pub mod error;
pub mod messages;
pub mod repository;
pub mod service;
pub mod transport;

pub use error::MailError;
pub use service::MailService;
pub use transport::OutgoingMail;
