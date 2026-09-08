//! OS keychain storage for connection passwords and API keys.
//!
//! `config.toml` used to store these as plain text. Instead of a new UUID field
//! per profile (extra migration surface), accounts are keyed by a composite of
//! already-stable fields — renaming a connection doesn't invalidate its entry.
//!
//! Every call here is best-effort: if no keychain backend is available (e.g. a
//! headless Linux box with no Secret Service daemon), storing/loading simply
//! fails and the caller falls back to the plain-text field already in the TOML.
//! This module never panics and never blocks the UI thread — `AppConfig::save()`/
//! `load()` (its only callers) already run outside the render loop.

use crate::config::{ConnectionProfile, SshTunnelConfig};

const SERVICE: &str = "ferox-pg";

/// Fixed account name for the single AI provider config (`AppConfig.ai` is a
/// struct, not a list, so no per-entry key is needed).
pub const AI_ACCOUNT: &str = "ai-api-key";

pub fn db_account(profile: &ConnectionProfile) -> String {
    format!("{}@{}:{}/{}", profile.user, profile.host, profile.port, profile.database)
}

pub fn ssh_account(ssh: &SshTunnelConfig) -> String {
    format!("ssh:{}@{}:{}", ssh.user, ssh.host, ssh.port)
}

/// Store `value` under `account` in the OS keychain. Returns `true` on success —
/// callers should only blank the plain-text field when this returns `true`.
pub fn store_secret(account: &str, value: &str) -> bool {
    keyring::Entry::new(SERVICE, account)
        .and_then(|e| e.set_password(value))
        .is_ok()
}

/// Load the secret for `account`, if the keychain has one.
pub fn load_secret(account: &str) -> Option<String> {
    keyring::Entry::new(SERVICE, account)
        .ok()?
        .get_password()
        .ok()
}

/// Best-effort removal — called when a connection profile is deleted so stale
/// credentials don't linger in the OS keychain indefinitely. Failure is silent:
/// there may simply be nothing stored (e.g. keychain was unavailable at save time).
pub fn delete_secret(account: &str) {
    if let Ok(entry) = keyring::Entry::new(SERVICE, account) {
        let _ = entry.delete_credential();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exercises the real OS keychain (Windows Credential Manager / macOS Keychain /
    /// Linux Secret Service) — `#[ignore]`d because CI runners and headless boxes
    /// don't reliably have a keychain backend available. Run explicitly with:
    /// `cargo test --lib secrets::tests::keychain_roundtrip -- --ignored`
    #[test]
    #[ignore]
    fn keychain_roundtrip() {
        let account = "ferox-test-account@localhost:5432/postgres";
        // Start from a clean slate in case a previous failed run left this behind.
        delete_secret(account);
        assert_eq!(load_secret(account), None, "keychain already had a stale entry");

        assert!(store_secret(account, "hunter2"), "store_secret failed — is a keychain backend available?");
        assert_eq!(load_secret(account), Some("hunter2".to_owned()));

        // Overwrite works too, not just first-write.
        assert!(store_secret(account, "hunter3"));
        assert_eq!(load_secret(account), Some("hunter3".to_owned()));

        delete_secret(account);
        assert_eq!(load_secret(account), None, "delete_secret did not remove the entry");
    }
}
