use super::repository::ApiKeysRepository;
use crate::modules::users::UsersService;

mod authenticate;
mod create_key;
mod list_keys;
mod revoke_key;

const MAX_ACTIVE_KEYS_PER_USER: i64 = 20;
/// Characters of the secret part shown as the key's recognisable prefix.
const DISPLAY_PREFIX_LEN: usize = 6;

#[derive(Clone)]
pub struct ApiKeysService {
    repo: ApiKeysRepository,
    users: UsersService,
}

impl ApiKeysService {
    pub fn new(repo: ApiKeysRepository, users: UsersService) -> Self {
        Self { repo, users }
    }
}
