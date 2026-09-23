//! Two-factor recovery codes: short, human-typeable, one-time-use codes for signing in when the
//! authenticator device is unavailable.

use rand::RngExt;

use crate::common::security::secret_token;

const RECOVERY_CODES_PER_ENROLLMENT: usize = 8;
/// Characters that cannot be confused for one another when read off a printed sheet (no `0`/`O`,
/// no `1`/`I`/`L`).
const ALPHABET: &[u8] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";
const CODE_LENGTH: usize = 10;

/// One freshly generated recovery code, in both forms: `raw` is shown to the user exactly once,
/// `hash` is what gets stored (the same hashing `secret_token` already uses for one-time links).
pub struct GeneratedRecoveryCode {
    pub raw: String,
    pub hash: String,
}

/// Generates a fresh batch of recovery codes, formatted like `XXXXX-XXXXX` for readability.
pub fn generate_batch() -> Vec<GeneratedRecoveryCode> {
    (0..RECOVERY_CODES_PER_ENROLLMENT)
        .map(|_| generate_one())
        .collect()
}

fn generate_one() -> GeneratedRecoveryCode {
    let mut rng = rand::rng();
    let random_characters: String = (0..CODE_LENGTH)
        .map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char)
        .collect();
    let (first_half, second_half) = random_characters.split_at(CODE_LENGTH / 2);
    let raw = format!("{first_half}-{second_half}");
    let hash = secret_token::hash(&raw);
    GeneratedRecoveryCode { raw, hash }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_the_expected_number_of_distinct_well_formed_codes() {
        let batch = generate_batch();
        assert_eq!(batch.len(), RECOVERY_CODES_PER_ENROLLMENT);
        for code in &batch {
            assert_eq!(code.raw.len(), CODE_LENGTH + 1); // + the separating hyphen
            assert_eq!(code.hash, secret_token::hash(&code.raw));
        }
        let unique_codes: std::collections::HashSet<_> = batch.iter().map(|c| &c.raw).collect();
        assert_eq!(
            unique_codes.len(),
            RECOVERY_CODES_PER_ENROLLMENT,
            "codes must be distinct"
        );
    }
}
