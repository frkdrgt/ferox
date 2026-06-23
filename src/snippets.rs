use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn snippets_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("pgclient")
        .join("snippets.toml")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snippet {
    pub name: String,
    pub sql: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Snippets {
    #[serde(default)]
    pub entries: Vec<Snippet>,
}

impl Snippets {
    pub fn load() -> Result<Self> {
        let path = snippets_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        Ok(toml::from_str(&content)?)
    }

    pub fn save(&self) -> Result<()> {
        let path = snippets_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, toml::to_string_pretty(self)?)?;
        Ok(())
    }

    /// Insert or overwrite by name (names are unique keys).
    pub fn upsert(&mut self, name: String, sql: String) {
        match self.entries.iter_mut().find(|s| s.name == name) {
            Some(existing) => existing.sql = sql,
            None => self.entries.push(Snippet { name, sql }),
        }
    }

    pub fn remove(&mut self, name: &str) {
        self.entries.retain(|s| s.name != name);
    }
}
