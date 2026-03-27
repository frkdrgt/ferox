use std::io::Write;
use std::path::PathBuf;

fn log_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("ferox").join("ferox.log"))
}

/// Call once at startup: creates log file, writes startup banner, installs panic hook.
pub fn setup() {
    // Ensure log directory exists.
    if let Some(path) = log_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        // Trim log file if it exceeds 1 MB to avoid unbounded growth.
        if let Ok(meta) = std::fs::metadata(&path) {
            if meta.len() > 1_048_576 {
                let _ = std::fs::write(&path, "");
            }
        }
    }

    // Enable backtraces so the panic hook can capture them.
    if std::env::var("RUST_BACKTRACE").is_err() {
        // SAFETY: single-threaded at this point, before tokio runtime starts.
        unsafe { std::env::set_var("RUST_BACKTRACE", "1") };
    }

    write_entry(
        "INFO",
        &format!(
            "=== Ferox started  v{}  |  os: {}  |  arch: {} ===",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH,
        ),
    );

    std::panic::set_hook(Box::new(|info| {
        // Location: file + line if available.
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown location".to_owned());

        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_owned()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "non-string panic payload".to_owned()
        };

        let bt = std::backtrace::Backtrace::capture();

        let msg = format!(
            "PANIC at {location}\n  message: {payload}\n\nBacktrace:\n{bt}"
        );

        write_entry("PANIC", &msg);

        // Also write to stderr — visible in debug builds / attached consoles.
        eprintln!("[ferox PANIC] {payload}  ({location})");
    }));
}

/// Write a single log entry with timestamp.
pub fn write_entry(level: &str, msg: &str) {
    let Some(path) = log_path() else { return };
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let entry = format!("[{ts}] [{level}] {msg}\n");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = f.write_all(entry.as_bytes());
    }
}

pub fn error(msg: &str) {
    write_entry("ERROR", msg);
    eprintln!("[ferox ERROR] {msg}");
}

/// Returns the path to the log file as a display string, or empty if unavailable.
pub fn log_file_path() -> String {
    log_path()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}
