use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub use crate::i18n::Lang;

fn default_ssh_port() -> u16 {
    22
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SshAuthMethod {
    Password(String),
    PrivateKey { path: String },
}

impl Default for SshAuthMethod {
    fn default() -> Self {
        SshAuthMethod::Password(String::new())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SshTunnelConfig {
    pub enabled: bool,
    pub host: String,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    pub user: String,
    #[serde(default)]
    pub auth: SshAuthMethod,
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("pgclient")
        .join("config.toml")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionProfile {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    #[serde(default)]
    pub password: String,
    pub database: String,
    #[serde(default)]
    pub ssl: SslMode,
    #[serde(default)]
    pub ssh_tunnel: Option<SshTunnelConfig>,
    /// Optional group label (e.g. "Production", "Staging", "Dev")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Explicit danger-tag color (RGB) — e.g. red for "Production" — shown on tabs
    /// and the connection switcher so destructive SQL isn't run against the wrong
    /// environment by accident. `None` falls back to a heuristic derived from `group`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
}

impl Default for ConnectionProfile {
    fn default() -> Self {
        Self {
            name: "New Connection".into(),
            host: "localhost".into(),
            port: 5432,
            user: "postgres".into(),
            password: String::new(),
            database: "postgres".into(),
            ssl: SslMode::Prefer,
            ssh_tunnel: None,
            group: None,
            color: None,
        }
    }
}

/// Fixed danger-tag swatches offered in the connection dialog — intentionally a
/// small fixed palette (not a full color wheel) so it stays visually consistent
/// with the heuristic colors used before this field existed (see `resolved_color`).
pub const DANGER_PALETTE: [[u8; 3]; 4] = [
    [200, 60, 60],  // red     — e.g. Production
    [220, 180, 50], // yellow  — e.g. Staging/Test
    [80, 200, 80],  // green   — e.g. Dev/Local
    [86, 156, 214], // blue    — neutral default
];

/// Fixed swatches offered for the `<null>` cell color. A plain `egui::Color32`
/// wheel (`color_edit_button_srgb`) opens its own popup `Area`, and nesting that
/// inside a `ui.menu_button` closes the whole Settings menu the instant the
/// picker is interacted with (a well-known egui nested-popup-in-menu quirk) —
/// a fixed row of plain buttons has no nested popup, so it sidesteps the bug.
pub const NULL_COLOR_PALETTE: [[u8; 3]; 6] = [
    [128, 100, 100], // muted red/brown — default
    [150, 150, 150], // gray
    [100, 130, 160], // muted blue
    [160, 100, 160], // muted purple
    [180, 140, 80],  // muted orange
    [90, 150, 90],   // muted green
];

impl ConnectionProfile {
    /// Resolve the danger-tag color for this connection: the explicit `color` tag
    /// if set, otherwise a heuristic guess from the `group` label (the exact
    /// heuristic used before the `color` field existed, kept as a fallback so
    /// existing users' saved profiles don't silently change appearance). `None`
    /// means "no tag" — callers that always want a color (e.g. a menu dot) should
    /// fall back to a neutral color themselves; callers drawing an accent stripe
    /// should skip it entirely on `None` so untagged connections stay unadorned.
    pub fn resolved_color(&self) -> Option<[u8; 3]> {
        if let Some(c) = self.color {
            return Some(c);
        }
        let g = self.group.as_deref()?.to_lowercase();
        if g.contains("prod") {
            Some([200, 60, 60])
        } else if g.contains("stag") || g.contains("test") {
            Some([220, 180, 50])
        } else if g.contains("dev") || g.contains("local") {
            Some([80, 200, 80])
        } else {
            None
        }
    }

    /// Build a connection string suitable for tokio-postgres.
    pub fn connection_string(&self) -> String {
        let ssl = match self.ssl {
            SslMode::Disable => "disable",
            SslMode::Prefer => "prefer",
            SslMode::Require => "require",
        };
        format!(
            "host={} port={} user={} password={} dbname={} sslmode={}",
            self.host, self.port, self.user, self.password, self.database, ssl
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SslMode {
    Disable,
    #[default]
    Prefer,
    Require,
}

impl std::fmt::Display for SslMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SslMode::Disable => write!(f, "disable"),
            SslMode::Prefer => write!(f, "prefer"),
            SslMode::Require => write!(f, "require"),
        }
    }
}

// ── AI provider config ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    /// "claude" | "groq" | "ollama" | "openai"
    #[serde(default = "AiConfig::default_provider")]
    pub provider: String,
    /// API key (empty = no auth, e.g. local Ollama)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub api_key: String,
    /// Model name override — empty = use provider default
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model: String,
    /// Base URL override — empty = use provider default
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub base_url: String,
}

impl AiConfig {
    fn default_provider() -> String { "claude".to_owned() }

    pub fn is_configured(&self) -> bool {
        match self.provider.as_str() {
            "ollama" => true,           // no key needed
            _ => !self.api_key.is_empty(),
        }
    }

    pub fn effective_base_url(&self) -> &str {
        if !self.base_url.is_empty() {
            return &self.base_url;
        }
        match self.provider.as_str() {
            "groq"   => "https://api.groq.com/openai/v1",
            "ollama" => "http://localhost:11434/v1",
            "openai" => "https://api.openai.com/v1",
            _        => "",   // claude uses its own URL
        }
    }

    pub fn effective_model(&self) -> &str {
        if !self.model.is_empty() {
            return &self.model;
        }
        match self.provider.as_str() {
            "claude" => "claude-haiku-4-5-20251001",
            "groq"   => "llama-3.3-70b-versatile",
            "ollama" => "llama3.2",
            "openai" => "gpt-4o-mini",
            _        => "gpt-4o-mini",
        }
    }
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: Self::default_provider(),
            api_key: String::new(),
            model: String::new(),
            base_url: String::new(),
        }
    }
}

// ── App config ────────────────────────────────────────────────────────────────

fn default_null_color() -> [u8; 3] {
    [128, 100, 100]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub connections: Vec<ConnectionProfile>,
    #[serde(default)]
    pub language: Lang,
    #[serde(default)]
    pub ai: AiConfig,
    /// RGB color used to render `<null>` cells in the result table.
    #[serde(default = "default_null_color")]
    pub null_color: [u8; 3],
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            connections: Vec::new(),
            language: Lang::default(),
            ai: AiConfig::default(),
            null_color: default_null_color(),
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let path = config_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        let mut config: Self = toml::from_str(&content)?;

        // Fill in passwords/keys that a previous `save()` migrated into the OS
        // keychain (see below) — this only touches runtime memory, never written
        // back to disk here. Legacy plain-text values already in the TOML are left
        // as-is; they migrate to the keychain the next time the user hits Save.
        for profile in &mut config.connections {
            if profile.password.is_empty() {
                if let Some(pw) = crate::secrets::load_secret(&crate::secrets::db_account(profile)) {
                    profile.password = pw;
                }
            }
            if let Some(ssh) = &mut profile.ssh_tunnel {
                let needs_load = matches!(&ssh.auth, SshAuthMethod::Password(p) if p.is_empty());
                if needs_load {
                    let account = crate::secrets::ssh_account(ssh);
                    if let Some(pw) = crate::secrets::load_secret(&account) {
                        ssh.auth = SshAuthMethod::Password(pw);
                    }
                }
            }
        }
        if config.ai.api_key.is_empty() {
            if let Some(key) = crate::secrets::load_secret(crate::secrets::AI_ACCOUNT) {
                config.ai.api_key = key;
            }
        }

        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Move secrets into the OS keychain before serializing. Only the disk
        // copy is redacted — `self` (used for the rest of the active session,
        // e.g. the open DB connection) is never touched. If the keychain write
        // fails (no backend available), the plain-text value is left in place
        // as a fallback so the user isn't silently locked out.
        let mut disk_copy = self.clone();
        for profile in &mut disk_copy.connections {
            if !profile.password.is_empty() {
                let account = crate::secrets::db_account(profile);
                if crate::secrets::store_secret(&account, &profile.password) {
                    profile.password.clear();
                }
            }
            if let Some(ssh) = &mut profile.ssh_tunnel {
                let account = crate::secrets::ssh_account(ssh);
                if let SshAuthMethod::Password(pw) = &mut ssh.auth {
                    if !pw.is_empty() && crate::secrets::store_secret(&account, pw) {
                        pw.clear();
                    }
                }
            }
        }
        if !disk_copy.ai.api_key.is_empty()
            && crate::secrets::store_secret(crate::secrets::AI_ACCOUNT, &disk_copy.ai.api_key)
        {
            disk_copy.ai.api_key.clear();
        }

        let content = toml::to_string_pretty(&disk_copy)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_color_prefers_explicit_tag() {
        let mut p = ConnectionProfile::default();
        p.group = Some("Production".into());
        p.color = Some([1, 2, 3]);
        assert_eq!(p.resolved_color(), Some([1, 2, 3]));
    }

    #[test]
    fn resolved_color_falls_back_to_group_heuristic() {
        let mut p = ConnectionProfile::default();
        p.group = Some("Production DB".into());
        assert_eq!(p.resolved_color(), Some([200, 60, 60]));

        p.group = Some("Staging".into());
        assert_eq!(p.resolved_color(), Some([220, 180, 50]));

        p.group = Some("local dev".into());
        assert_eq!(p.resolved_color(), Some([80, 200, 80]));
    }

    #[test]
    fn resolved_color_none_when_untagged_and_ungrouped() {
        let p = ConnectionProfile::default();
        assert_eq!(p.resolved_color(), None);

        let mut p2 = ConnectionProfile::default();
        p2.group = Some("Analytics".into()); // doesn't match any heuristic keyword
        assert_eq!(p2.resolved_color(), None);
    }
}
