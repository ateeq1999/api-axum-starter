use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum Provider {
    Google,
    Github,
}

impl Provider {
    pub const ALL: [Provider; 2] = [Provider::Google, Provider::Github];

    pub fn as_str(self) -> &'static str {
        match self {
            Provider::Google => "google",
            Provider::Github => "github",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Provider::Google => "Google",
            Provider::Github => "GitHub",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.as_str() == name)
    }

    /// Only what we need: identity and a verified email address.
    pub fn scopes(self) -> &'static str {
        match self {
            Provider::Google => "openid email profile",
            Provider::Github => "read:user user:email",
        }
    }
}
