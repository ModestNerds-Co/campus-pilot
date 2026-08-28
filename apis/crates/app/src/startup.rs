//! Enforces database-backed deployment invariants before the HTTP service starts.
//!
//! Startup permits an absent AI-provider keyring only while no active encrypted
//! credentials exist. Stored credential material is never loaded by this check.
//!
//! Copyright (c) 2026 Codecraft Solutions. All rights reserved.

use anyhow::{Context, Result, bail};
use cp_ai_providers::CredentialKeyring;
use sqlx::PgPool;

/// Proves that every active stored AI-provider credential remains decryptable.
pub async fn validate_ai_provider_credential_keyring(
    pool: &PgPool,
    keyring: Option<&CredentialKeyring>,
) -> Result<()> {
    let stored_key_ids = sqlx::query_scalar::<_, String>(
        r#"
        SELECT DISTINCT credential_key_id
        FROM ai_provider_connections
        WHERE deleted_at IS NULL
          AND credential_key_id IS NOT NULL
        ORDER BY credential_key_id
        "#,
    )
    .fetch_all(pool)
    .await
    .context("inspect active AI provider credential key identifiers")?;

    validate_stored_key_ids(&stored_key_ids, keyring)
}

fn validate_stored_key_ids(
    stored_key_ids: &[String],
    keyring: Option<&CredentialKeyring>,
) -> Result<()> {
    if stored_key_ids.is_empty() {
        return Ok(());
    }

    let keyring = keyring
        .context("active AI provider credentials exist, but no credential keyring is configured")?;
    let missing_key_ids = stored_key_ids
        .iter()
        .filter(|key_id| !keyring.contains_key_id(key_id))
        .map(String::as_str)
        .collect::<Vec<_>>();

    if !missing_key_ids.is_empty() {
        bail!(
            "AI provider credential keyring is missing stored key identifiers: {}",
            missing_key_ids.join(", ")
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use cp_ai_providers::CredentialKeyring;

    use super::validate_stored_key_ids;

    fn keyring() -> CredentialKeyring {
        CredentialKeyring::from_base64(
            BTreeMap::from([
                ("current".to_owned(), STANDARD.encode([7_u8; 32])),
                ("previous".to_owned(), STANDARD.encode([9_u8; 32])),
            ]),
            "current",
        )
        .unwrap()
    }

    #[test]
    fn no_stored_credentials_allow_an_unconfigured_keyring() {
        validate_stored_key_ids(&[], None).unwrap();
    }

    #[test]
    fn stored_credentials_require_a_keyring_covering_rotation_history() {
        let stored_key_ids = vec!["current".to_owned(), "previous".to_owned()];
        validate_stored_key_ids(&stored_key_ids, Some(&keyring())).unwrap();

        let unconfigured = validate_stored_key_ids(&stored_key_ids, None).unwrap_err();
        assert_eq!(
            unconfigured.to_string(),
            "active AI provider credentials exist, but no credential keyring is configured"
        );

        let missing_key_ids = vec!["current".to_owned(), "retired".to_owned()];
        let missing = validate_stored_key_ids(&missing_key_ids, Some(&keyring())).unwrap_err();
        assert_eq!(
            missing.to_string(),
            "AI provider credential keyring is missing stored key identifiers: retired"
        );
    }
}
