use std::sync::mpsc::{Receiver, Sender};

use anyhow::Result;

use crate::config::AiConfig;

// ── Commands UI → AI thread ────────────────────────────────────────────────────

#[derive(Debug)]
pub enum AiCommand {
    /// Generate SQL from a natural-language prompt.
    NlToSql {
        prompt: String,
        /// Compact schema summary built by Sidebar::schema_context_for_ai().
        schema_context: String,
    },
    /// Hot-update AI config without restarting the thread.
    SetConfig(AiConfig),
}

// ── Events AI thread → UI ──────────────────────────────────────────────────────

#[derive(Debug)]
pub enum AiEvent {
    /// Request dispatched; UI should show spinner.
    Thinking,
    /// AI returned a SQL string.
    SqlGenerated(String),
    /// AI call failed.
    Error(String),
}

// ── Handle ────────────────────────────────────────────────────────────────────

pub struct AiHandle;

impl AiHandle {
    pub fn spawn(config: AiConfig, cmd_rx: Receiver<AiCommand>, evt_tx: Sender<AiEvent>) {
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("ai tokio rt");
            rt.block_on(ai_worker(config, cmd_rx, evt_tx));
        });
    }
}

// ── Worker ─────────────────────────────────────────────────────────────────────

async fn ai_worker(initial_cfg: AiConfig, cmd_rx: Receiver<AiCommand>, evt_tx: Sender<AiEvent>) {
    use std::sync::{Arc, Mutex};

    let cmd_rx = Arc::new(Mutex::new(cmd_rx));
    let mut cfg = initial_cfg;

    loop {
        let rx = Arc::clone(&cmd_rx);
        let cmd = match tokio::task::spawn_blocking(move || rx.lock().unwrap().recv()).await {
            Ok(Ok(c)) => c,
            _ => break,
        };

        match cmd {
            AiCommand::NlToSql { prompt, schema_context } => {
                let _ = evt_tx.send(AiEvent::Thinking);
                match call_ai(&cfg, &prompt, &schema_context).await {
                    Ok(sql) => { let _ = evt_tx.send(AiEvent::SqlGenerated(sql)); }
                    Err(e)  => { let _ = evt_tx.send(AiEvent::Error(e.to_string())); }
                }
            }
            AiCommand::SetConfig(new_cfg) => {
                cfg = new_cfg;
            }
        }
    }
}

// ── Dispatch to correct provider ───────────────────────────────────────────────

async fn call_ai(cfg: &AiConfig, prompt: &str, schema_context: &str) -> Result<String> {
    if !cfg.is_configured() {
        anyhow::bail!(
            "AI not configured. Set provider + API key via Settings → AI."
        );
    }

    let system = "\
You are a senior PostgreSQL expert. Generate exactly ONE valid PostgreSQL query based on the user's request.

Rules:
- Output ONLY the SQL query. No explanations, no comments, no markdown, no code fences.
- The query MUST be syntactically correct and executable in PostgreSQL.
- Use ONLY the tables and columns provided in the schema. Never invent names.
- If the request references non-existent tables or columns, do not guess — return: SELECT 1;
- If the request cannot be fulfilled with the given schema, return: SELECT 1;
- Prefer explicit column names over SELECT *.
- Always qualify columns with table aliases when multiple tables are involved.
- Use appropriate JOIN types (INNER, LEFT) based on context. Prefer JOINs over subqueries.
- Apply LIMIT when the result set could be large, unless the user explicitly asks for all data.
- Use ORDER BY with deterministic columns (id, created_at, etc.) when LIMIT is applied.
- Ensure correct aggregations: always include GROUP BY when using aggregate functions.
- Use PostgreSQL-specific features when appropriate (ILIKE, RETURNING, window functions, CTEs).
- Handle NULLs safely (use COALESCE or IS NOT NULL where relevant).
- Avoid scanning entire large tables without WHERE filters unless explicitly requested.
- Do NOT perform destructive operations (DELETE, DROP, TRUNCATE, UPDATE) unless explicitly requested.
- Prefer indexed columns for filtering when inferable from column names (id, *_id, created_at, status).";

    let user_msg = if schema_context.is_empty() {
        prompt.to_owned()
    } else {
        format!("Schema:\n{schema_context}\n\nQuery: {prompt}")
    };

    match cfg.provider.as_str() {
        "claude" => call_claude(cfg, system, &user_msg).await,
        _        => call_openai_compat(cfg, system, &user_msg).await,
    }
}

// ── Anthropic Claude ───────────────────────────────────────────────────────────

const CLAUDE_API_URL: &str = "https://api.anthropic.com/v1/messages";

async fn call_claude(cfg: &AiConfig, system: &str, user_msg: &str) -> Result<String> {
    let body = serde_json::json!({
        "model": cfg.effective_model(),
        "max_tokens": 1024,
        "system": system,
        "messages": [{ "role": "user", "content": user_msg }]
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(CLAUDE_API_URL)
        .header("x-api-key", &cfg.api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    let json: serde_json::Value = resp.json().await?;

    if !status.is_success() {
        let msg = json["error"]["message"]
            .as_str()
            .unwrap_or("unknown error")
            .to_owned();
        anyhow::bail!("Claude API {status}: {msg}");
    }

    extract_text(&json, "content[0].text")
}

// ── OpenAI-compatible (Groq / Ollama / OpenAI / OpenRouter / custom) ───────────

async fn call_openai_compat(cfg: &AiConfig, system: &str, user_msg: &str) -> Result<String> {
    let base = cfg.effective_base_url();
    let url = format!("{base}/chat/completions");

    let body = serde_json::json!({
        "model": cfg.effective_model(),
        "max_tokens": 1024,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user",   "content": user_msg }
        ]
    });

    let client = reqwest::Client::new();
    let mut req = client.post(&url).json(&body);

    if !cfg.api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", cfg.api_key));
    }

    let resp = req.send().await?;
    let status = resp.status();
    let json: serde_json::Value = resp.json().await?;

    if !status.is_success() {
        let msg = json["error"]["message"]
            .as_str()
            .unwrap_or("unknown error")
            .to_owned();
        anyhow::bail!("{} API {status}: {msg}", cfg.provider);
    }

    extract_text(&json, "choices[0].message.content")
}

// ── Helper ─────────────────────────────────────────────────────────────────────

/// Navigate a dotted path like `"choices[0].message.content"` into a JSON value.
fn extract_text(json: &serde_json::Value, path: &str) -> Result<String> {
    let mut cur = json;
    for part in path.split('.') {
        if let Some(idx_start) = part.find('[') {
            let key = &part[..idx_start];
            let idx: usize = part[idx_start + 1..part.len() - 1].parse()?;
            if !key.is_empty() {
                cur = &cur[key];
            }
            cur = &cur[idx];
        } else {
            cur = &cur[part];
        }
    }
    cur.as_str()
        .map(|s| s.trim().to_owned())
        .ok_or_else(|| anyhow::anyhow!("Unexpected AI response format"))
}
