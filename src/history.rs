use anyhow::Result;
use std::path::PathBuf;

const MAX_HISTORY: usize = 500;

fn history_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("pgclient")
        .join("history.txt")
}

#[derive(Debug, Default)]
pub struct QueryHistory {
    entries: Vec<String>,
    /// Current navigation index (None = not browsing history)
    pub cursor: Option<usize>,
}

impl QueryHistory {
    pub fn load() -> Result<Self> {
        let path = history_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        let entries: Vec<String> = content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.replace("\\n", "\n"))
            .collect();
        Ok(Self {
            entries,
            cursor: None,
        })
    }

    pub fn save(&self) -> Result<()> {
        let path = history_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content: String = self
            .entries
            .iter()
            .map(|e| e.replace('\n', "\\n"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn push(&mut self, query: String) {
        // Avoid duplicate consecutive entries
        if self.entries.last().map(|e| e.as_str()) != Some(&query) {
            self.entries.push(query);
        }
        if self.entries.len() > MAX_HISTORY {
            self.entries.remove(0);
        }
        self.cursor = None;
    }

    pub fn prev(&mut self) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        let idx = match self.cursor {
            None => self.entries.len().saturating_sub(1),
            Some(0) => 0,
            Some(i) => i - 1,
        };
        self.cursor = Some(idx);
        self.entries.get(idx).map(|s| s.as_str())
    }

    pub fn next(&mut self) -> Option<&str> {
        let idx = self.cursor? + 1;
        if idx >= self.entries.len() {
            self.cursor = None;
            return None;
        }
        self.cursor = Some(idx);
        self.entries.get(idx).map(|s| s.as_str())
    }

    pub fn all(&self) -> &[String] {
        &self.entries
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        let q = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| e.to_lowercase().contains(&q))
            .map(|s| s.as_str())
            .collect()
    }
}
