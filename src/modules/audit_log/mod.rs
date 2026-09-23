//! Durable, admin-readable log of security-sensitive administrative actions (role changes,
//! deactivation, deletion) — see `service::action` for the recorded action names. Written to by
//! `modules::users`; read through `GET /api/v1/audit-log` (admin only).

pub mod controller;
pub mod dto;
pub mod entity;
pub mod repository;
pub mod service;

pub use controller::router;
pub use repository::AuditLogRepository;
pub use service::{AuditLogService, action};
