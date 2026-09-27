//! All SQL for users, one file per responsibility. Every file adds methods to the same
//! `UsersRepository`, so callers see one type.

use sqlx::PgPool;

use super::{
    dto::{SortOrder, UserSort},
    error::UsersError,
};
use crate::common::{error::AppError, security::Role};

/// Every column a `User` is built from, shared by each `SELECT` and `RETURNING`.
/// Declared before the `mod` lines so every file below can use it.
macro_rules! columns {
    () => {
        "id, email, password_hash, display_name, role, is_active, email_verified_at, created_at, updated_at, deleted_at, avatar_key, password_set, token_version, failed_login_attempts, locked_until, totp_secret, totp_enabled"
    };
}

mod accounts;
mod admin_changes;
mod avatar;
mod credentials;
mod email;
mod listing;
mod sign_in_methods;
mod totp;

pub struct NewUser<'a> {
    pub email: &'a str,
    pub password_hash: &'a str,
    pub display_name: Option<&'a str>,
    pub role: Role,
    pub email_verified: bool,
    /// `false` for accounts created through OAuth that have no usable password.
    pub password_set: bool,
}

#[derive(Default)]
pub struct UserPatch<'a> {
    pub display_name: Option<&'a str>,
    pub role: Option<Role>,
    pub is_active: Option<bool>,
}

pub struct UserFilter<'a> {
    pub search: Option<&'a str>,
    pub sort: UserSort,
    pub order: SortOrder,
    pub limit: i64,
    pub offset: i64,
}

/// Arbitrary but fixed key for the Postgres advisory lock guarding the "at least one active
/// admin remains" invariant (the only advisory lock this app takes, so any value works). A plain
/// row lock cannot guard this: two concurrent requests demoting *different* admins would each
/// lock the *other* admin's row, so neither blocks the other, and both count checks could still
/// pass right before both writes land, leaving zero admins. The advisory lock is not tied to any
/// row, so every such transaction serializes against every other one, closing that race.
const ADMIN_GUARD_KEY: i64 = 8_412_017;

#[derive(Clone)]
pub struct UsersRepository {
    db: PgPool,
}

impl UsersRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }
}

fn map_write_error(error: sqlx::Error) -> AppError {
    match error {
        sqlx::Error::Database(e) if e.is_unique_violation() => UsersError::EmailTaken.into(),
        other => other.into(),
    }
}
