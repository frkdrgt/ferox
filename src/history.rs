use anyhow::Result;
use chrono::TimeZone;
use std::path::PathBuf;

const MAX_HISTORY: usize = 500;

fn history_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("pgclient")
        .join("history.txt")
}

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub sql: String,
    pub executed_at: chrono::DateTime<chrono::Local>,
}

impl HistoryEntry {
    fn new(sql: String) -> Self {
        Self { sql, executed_at: chrono::Local::now() }
    }
}

#[derive(Debug, Default)]
pub struct QueryHistory {
    entries: Vec<HistoryEntry>,
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
        let entries: Vec<HistoryEntry> = content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                // New format: "YYYY-MM-DDTHH:MM:SS\tSQL"
                if let Some(tab) = l.find('\t') {
                    let ts = &l[..tab];
                    let sql = l[tab + 1..].replace("\\n", "\n");
                    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S")
                    {
                        let dt = chrono::Local::from_local_datetime(&chrono::Local, &dt)
                            .single()
                            .unwrap_or_else(chrono::Local::now);
                        return HistoryEntry { sql, executed_at: dt };
                    }
                }
                // Legacy format: plain SQL line
                HistoryEntry::new(l.replace("\\n", "\n"))
            })
            .collect();
        Ok(Self { entries, cursor: None })
    }

    pub fn save(&self) -> Result<()> {
        let path = history_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content: String = self
            .entries
            .iter()
            .map(|e| {
                let ts = e.executed_at.format("%Y-%m-%dT%H:%M:%S");
                let sql = e.sql.replace('\n', "\\n");
                format!("{ts}\t{sql}")
            })
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn push(&mut self, query: String) {
        let last_sql = self.entries.last().map(|e| e.sql.as_str());
        if last_sql != Some(&query) {
            self.entries.push(HistoryEntry::new(query));
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
        self.entries.get(idx).map(|e| e.sql.as_str())
    }

    pub fn next(&mut self) -> Option<&str> {
        let idx = self.cursor? + 1;
        if idx >= self.entries.len() {
            self.cursor = None;
            return None;
        }
        self.cursor = Some(idx);
        self.entries.get(idx).map(|e| e.sql.as_str())
    }

    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    pub fn all(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.sql.as_str()).collect()
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        let q = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| e.sql.to_lowercase().contains(&q))
            .map(|e| e.sql.as_str())
            .collect()
    }
}
