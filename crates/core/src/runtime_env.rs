//! External runtime environment file.
//!
//! Lets users change `WF_*` / `CONNECT_*` runtime configuration without
//! rebuilding the app or opening a terminal: edit a user-owned `.env`-style
//! file, restart the app, done.
//!
//! Where the file lives (first existing candidate wins):
//!
//! 1. `$WF_ENV_FILE`, when set to an existing file (explicit override; a
//!    missing path is a startup error, not a silent skip).
//! 2. `<app-support>/wealthfolio.env` — the primary user-editable location.
//!    On macOS this is `~/Library/Application Support/com.teymz.wealthfolio/`
//!    (`$WF_APP_SUPPORT_DIR` overrides the directory, mainly for tests).
//!    It lives **outside** the `.app` bundle, so it survives app updates.
//!    (A file next to the binary would sit inside the signed bundle and be
//!    wiped on every update; a launchd plist only applies to daemons, not
//!    GUI apps launched from Finder.)
//! 3. `<executable-dir>/wealthfolio.env` — sidecar fallback for portable and
//!    development layouts.
//!
//! Precedence (highest first): process environment > CWD `.env` (loaded by
//! each binary's existing `dotenvy` call *after* [`load`]) > this file >
//! compile-time baked values (`option_env!`) > built-in defaults.
//!
//! Rules: a missing file is fine (process env and defaults apply); a
//! malformed line is a startup error naming `file:line` (values are never
//! echoed); only managed keys (`WF_*`, `CONNECT_*`, `RUST_LOG`) are applied,
//! everything else is ignored; a key already present in the process
//! environment (even empty) is left untouched. Resolved sources are logged
//! per key at debug level — keys only, never values.

use std::path::{Path, PathBuf};

/// File name looked up in the app-support and executable directories.
pub const ENV_FILE_NAME: &str = "wealthfolio.env";
/// Explicit override: when set, this exact file is used.
pub const EXPLICIT_PATH_VAR: &str = "WF_ENV_FILE";
/// Test/portability hook: overrides the app-support directory.
pub const SUPPORT_DIR_OVERRIDE_VAR: &str = "WF_APP_SUPPORT_DIR";
/// Matches the Tauri bundle identifier (`apps/tauri/tauri.conf.json`).
pub const APP_IDENTIFIER: &str = "com.teymz.wealthfolio";

/// Where a resolved value came from. [`std::fmt::Display`] renders the
/// lowercase name used in debug logs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvSource {
    Process,
    File,
}

impl std::fmt::Display for EnvSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnvSource::Process => write!(f, "process"),
            EnvSource::File => write!(f, "file"),
        }
    }
}

/// One managed key seen while loading, and where its value came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedEntry {
    pub key: String,
    pub source: EnvSource,
}

/// Outcome of [`load`] / [`load_from`]: which file was used (if any) and the
/// per-key sources. Contains no values, so it is safe to debug-log.
#[derive(Clone, Debug, Default)]
pub struct LoadReport {
    pub path: Option<PathBuf>,
    pub entries: Vec<LoadedEntry>,
}

/// Directories searched for [`ENV_FILE_NAME`]. [`load`] uses the real ones;
/// tests inject temporary directories through [`load_from`].
#[derive(Clone, Debug, Default)]
pub struct SearchDirs {
    pub exe_dir: Option<PathBuf>,
    pub support_dir: Option<PathBuf>,
}

impl SearchDirs {
    /// Directories for the running process.
    pub fn current() -> Self {
        Self {
            exe_dir: std::env::current_exe()
                .ok()
                .and_then(|path| path.parent().map(Path::to_path_buf)),
            support_dir: support_dir(),
        }
    }
}

/// Only these keys are ever applied from the file, so a stray
/// `PATH=...` line cannot hijack the process.
pub fn is_managed_key(key: &str) -> bool {
    key.starts_with("WF_") || key.starts_with("CONNECT_") || key == "RUST_LOG"
}

/// App-support directory holding [`ENV_FILE_NAME`]. Honours
/// [`SUPPORT_DIR_OVERRIDE_VAR`]; otherwise the per-OS conventional location
/// for [`APP_IDENTIFIER`].
pub fn support_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os(SUPPORT_DIR_OVERRIDE_VAR).filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .filter(|v| !v.is_empty())
            .map(|home| {
                PathBuf::from(home)
                    .join("Library")
                    .join("Application Support")
                    .join(APP_IDENTIFIER)
            })
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA")
            .filter(|v| !v.is_empty())
            .map(|dir| PathBuf::from(dir).join(APP_IDENTIFIER))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .filter(|v| !v.is_empty())
                    .map(|home| PathBuf::from(home).join(".config"))
            })
            .map(|dir| dir.join(APP_IDENTIFIER))
    }
}

fn resolve_path(search: &SearchDirs) -> anyhow::Result<Option<PathBuf>> {
    if let Some(explicit) = std::env::var_os(EXPLICIT_PATH_VAR).filter(|v| !v.is_empty()) {
        let path = PathBuf::from(explicit);
        if !path.is_file() {
            anyhow::bail!(
                "{EXPLICIT_PATH_VAR} points to '{}', which does not exist or is not a file",
                path.display()
            );
        }
        return Ok(Some(path));
    }
    let mut candidates = Vec::with_capacity(2);
    if let Some(dir) = search.support_dir.as_ref() {
        candidates.push(dir.join(ENV_FILE_NAME));
    }
    if let Some(dir) = search.exe_dir.as_ref() {
        candidates.push(dir.join(ENV_FILE_NAME));
    }
    Ok(candidates.into_iter().find(|path| path.is_file()))
}

fn is_valid_key(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn unescape_double_quoted(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some(next) => out.push(next),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn strip_quotes(value: &str, path: &Path, line_no: usize) -> anyhow::Result<String> {
    for quote in ['"', '\''] {
        if value.starts_with(quote) {
            if value.len() < 2 || !value.ends_with(quote) {
                anyhow::bail!("{}:{line_no}: unterminated {quote} quote", path.display());
            }
            let inner = &value[quote.len_utf8()..value.len() - quote.len_utf8()];
            if quote == '"' {
                return Ok(unescape_double_quoted(inner));
            }
            return Ok(inner.to_string());
        }
    }
    Ok(value.to_string())
}

/// Parse `.env`-style content. Errors name `file:line` and never echo values.
pub fn parse_env_file(path: &Path, content: &str) -> anyhow::Result<Vec<(String, String)>> {
    let mut pairs = Vec::new();
    for (index, raw_line) in content.lines().enumerate() {
        let line_no = index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line
            .strip_prefix("export ")
            .map_or(line, |rest| rest.trim_start());
        let Some(eq) = line.find('=') else {
            anyhow::bail!(
                "{}:{line_no}: malformed line (expected KEY=VALUE)",
                path.display()
            );
        };
        let key = line[..eq].trim();
        if !is_valid_key(key) {
            anyhow::bail!(
                "{}:{line_no}: invalid key '{key}' (expected [A-Za-z_][A-Za-z0-9_]*)",
                path.display()
            );
        }
        let value = strip_quotes(line[eq + 1..].trim(), path, line_no)?;
        pairs.push((key.to_string(), value));
    }
    Ok(pairs)
}

/// Apply parsed pairs to the process environment. Present process vars win
/// (even when empty); later file entries win over earlier ones for the same
/// key. Returns one entry per managed key (keys only, never values).
pub fn apply(pairs: Vec<(String, String)>) -> Vec<LoadedEntry> {
    // Last file occurrence wins; iterate in reverse keeping first sighting.
    let mut ordered: Vec<(String, String)> = Vec::with_capacity(pairs.len());
    let mut seen = std::collections::HashSet::new();
    for (key, value) in pairs.into_iter().rev() {
        if seen.insert(key.clone()) {
            ordered.push((key, value));
        }
    }
    ordered.reverse();

    let mut entries = Vec::new();
    for (key, value) in ordered {
        if !is_managed_key(&key) {
            log::debug!("runtime-env: ignoring unmanaged key '{key}'");
            continue;
        }
        if std::env::var_os(&key).is_some() {
            entries.push(LoadedEntry {
                key,
                source: EnvSource::Process,
            });
        } else {
            std::env::set_var(&key, value);
            entries.push(LoadedEntry {
                key,
                source: EnvSource::File,
            });
        }
    }
    entries
}

/// Emit the per-key sources at debug level (keys only, never values).
pub fn log_report(report: &LoadReport) {
    match &report.path {
        Some(path) => log::debug!(
            "runtime-env: {} managed keys resolved from '{}'",
            report.entries.len(),
            path.display()
        ),
        None => log::debug!("runtime-env: no external env file; process env and defaults apply"),
    }
    for entry in &report.entries {
        log::debug!(
            "runtime-env: '{}' resolved from {}",
            entry.key,
            entry.source
        );
    }
}

static LAST_REPORT: std::sync::OnceLock<LoadReport> = std::sync::OnceLock::new();

/// Re-emit the last [`load`] report. Call after the tracing/log backend is
/// installed: [`load`] runs before that during startup, so its own debug logs
/// would otherwise be dropped.
pub fn log_last_report() {
    if let Some(report) = LAST_REPORT.get() {
        log_report(report);
    }
}

/// Load with injected directories (tests, tooling). Does not touch the
/// process-global report; see [`load`].
pub fn load_from(search: &SearchDirs) -> anyhow::Result<LoadReport> {
    let Some(path) = resolve_path(search)? else {
        return Ok(LoadReport::default());
    };
    let content = std::fs::read_to_string(&path).map_err(|error| {
        anyhow::anyhow!("cannot read runtime env file '{}': {error}", path.display())
    })?;
    let pairs = parse_env_file(&path, &content)?;
    let entries = apply(pairs);
    let report = LoadReport {
        path: Some(path),
        entries,
    };
    log_report(&report);
    Ok(report)
}

/// Load with the real process directories and remember the report for
/// [`log_last_report`]. Missing file is fine; malformed lines error.
pub fn load() -> anyhow::Result<LoadReport> {
    let report = load_from(&SearchDirs::current())?;
    let _ = LAST_REPORT.set(report.clone());
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::sync::{Mutex, MutexGuard};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn lock_env() -> MutexGuard<'static, ()> {
        ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Saves and restores one process var around a test.
    struct EnvGuard {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl EnvGuard {
        fn hold(key: &'static str) -> Self {
            Self {
                key,
                previous: std::env::var_os(key),
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }

    fn search_in(dir: &Path) -> SearchDirs {
        SearchDirs {
            exe_dir: None,
            support_dir: Some(dir.to_path_buf()),
        }
    }

    #[test]
    fn parses_supported_syntax() {
        let path = Path::new("wealthfolio.env");
        let pairs = parse_env_file(
            path,
            "# comment\nWF_LISTEN_ADDR=127.0.0.1:8088\nexport CONNECT_API_URL=https://x.example\nWF_QUOTED=\"a=b # kept\"\nWF_SINGLE='sp ace'\nWF_EMPTY=\n",
        )
        .unwrap();
        assert_eq!(
            pairs,
            vec![
                ("WF_LISTEN_ADDR".into(), "127.0.0.1:8088".into()),
                ("CONNECT_API_URL".into(), "https://x.example".into()),
                ("WF_QUOTED".into(), "a=b # kept".into()),
                ("WF_SINGLE".into(), "sp ace".into()),
                ("WF_EMPTY".into(), String::new()),
            ]
        );
    }

    #[test]
    fn malformed_lines_name_file_and_line_without_values() {
        let path = Path::new("/tmp/wealthfolio.env");
        for (line_no, content) in [
            (1, "just some words\n"),
            (2, "WF_OK=1\n0BAD=oops\n"),
            (2, "WF_OK=1\nWF_UNTERMINATED=\"oops\n"),
        ] {
            let error = parse_env_file(path, content).unwrap_err();
            let message = format!("{error:#}");
            assert!(
                message.contains(&format!("/tmp/wealthfolio.env:{line_no}:")),
                "missing file:line in: {message}"
            );
            assert!(!message.contains("oops"), "value leaked in: {message}");
        }
    }

    #[test]
    fn file_values_apply_and_process_env_wins() {
        let _lock = lock_env();
        let from_file = EnvGuard::hold("WF_RUNTIME_ENV_TEST_FILE");
        let from_process = EnvGuard::hold("WF_RUNTIME_ENV_TEST_PROCESS");
        let unmanaged = EnvGuard::hold("UNMANAGED_RUNTIME_ENV_TEST");
        let _ = (from_file, from_process, unmanaged);

        std::env::set_var("WF_RUNTIME_ENV_TEST_PROCESS", "process-value");
        std::env::remove_var("WF_RUNTIME_ENV_TEST_FILE");
        std::env::remove_var("UNMANAGED_RUNTIME_ENV_TEST");

        let entries = apply(vec![
            ("WF_RUNTIME_ENV_TEST_FILE".into(), "file-value".into()),
            (
                "WF_RUNTIME_ENV_TEST_FILE".into(),
                "file-value-latest".into(),
            ),
            ("WF_RUNTIME_ENV_TEST_PROCESS".into(), "file-value".into()),
            ("UNMANAGED_RUNTIME_ENV_TEST".into(), "file-value".into()),
        ]);

        // Last file occurrence wins; unmanaged keys are skipped entirely.
        assert_eq!(
            std::env::var("WF_RUNTIME_ENV_TEST_FILE").unwrap(),
            "file-value-latest"
        );
        assert_eq!(
            std::env::var("WF_RUNTIME_ENV_TEST_PROCESS").unwrap(),
            "process-value"
        );
        assert!(std::env::var_os("UNMANAGED_RUNTIME_ENV_TEST").is_none());
        assert_eq!(
            entries,
            vec![
                LoadedEntry {
                    key: "WF_RUNTIME_ENV_TEST_FILE".into(),
                    source: EnvSource::File,
                },
                LoadedEntry {
                    key: "WF_RUNTIME_ENV_TEST_PROCESS".into(),
                    source: EnvSource::Process,
                },
            ]
        );
    }

    #[test]
    fn empty_process_value_still_wins() {
        let _lock = lock_env();
        let _guard = EnvGuard::hold("WF_RUNTIME_ENV_TEST_EMPTY");
        std::env::set_var("WF_RUNTIME_ENV_TEST_EMPTY", "");
        let entries = apply(vec![(
            "WF_RUNTIME_ENV_TEST_EMPTY".into(),
            "file-value".into(),
        )]);
        assert_eq!(std::env::var("WF_RUNTIME_ENV_TEST_EMPTY").unwrap(), "");
        assert_eq!(entries[0].source, EnvSource::Process);
    }

    #[test]
    fn missing_file_is_fine() {
        let dir = tempfile::tempdir().unwrap();
        let report = load_from(&search_in(dir.path())).unwrap();
        assert!(report.path.is_none());
        assert!(report.entries.is_empty());
    }

    #[test]
    fn support_dir_file_loads_and_restart_picks_up_edits() {
        let _lock = lock_env();
        let _key = EnvGuard::hold("WF_RUNTIME_ENV_TEST_RELOAD");
        let _override = EnvGuard::hold(SUPPORT_DIR_OVERRIDE_VAR);
        let _explicit = EnvGuard::hold(EXPLICIT_PATH_VAR);
        std::env::remove_var("WF_RUNTIME_ENV_TEST_RELOAD");
        std::env::remove_var(EXPLICIT_PATH_VAR);

        let dir = tempfile::tempdir().unwrap();
        std::env::set_var(SUPPORT_DIR_OVERRIDE_VAR, dir.path());
        let path = dir.path().join(ENV_FILE_NAME);
        std::fs::write(&path, "WF_RUNTIME_ENV_TEST_RELOAD=one\n").unwrap();

        let first = load_from(&SearchDirs {
            exe_dir: None,
            support_dir: Some(dir.path().to_path_buf()),
        })
        .unwrap();
        assert_eq!(first.path.as_deref(), Some(path.as_path()));
        assert_eq!(std::env::var("WF_RUNTIME_ENV_TEST_RELOAD").unwrap(), "one");

        // A file edit is picked up by the next load (callers reload on restart).
        std::env::remove_var("WF_RUNTIME_ENV_TEST_RELOAD");
        std::fs::write(&path, "WF_RUNTIME_ENV_TEST_RELOAD=two\n").unwrap();
        load_from(&SearchDirs {
            exe_dir: None,
            support_dir: Some(dir.path().to_path_buf()),
        })
        .unwrap();
        assert_eq!(std::env::var("WF_RUNTIME_ENV_TEST_RELOAD").unwrap(), "two");
    }

    #[test]
    fn explicit_path_wins_and_missing_explicit_path_errors() {
        let _lock = lock_env();
        let _key = EnvGuard::hold("WF_RUNTIME_ENV_TEST_EXPLICIT");
        let _override = EnvGuard::hold(SUPPORT_DIR_OVERRIDE_VAR);
        let _explicit = EnvGuard::hold(EXPLICIT_PATH_VAR);
        std::env::remove_var("WF_RUNTIME_ENV_TEST_EXPLICIT");

        let support = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        std::fs::write(
            support.path().join(ENV_FILE_NAME),
            "WF_RUNTIME_ENV_TEST_EXPLICIT=support\n",
        )
        .unwrap();
        let explicit = other.path().join("custom.env");
        std::fs::write(&explicit, "WF_RUNTIME_ENV_TEST_EXPLICIT=explicit\n").unwrap();
        std::env::set_var(EXPLICIT_PATH_VAR, &explicit);

        let report = load_from(&search_in(support.path())).unwrap();
        assert_eq!(report.path.as_deref(), Some(explicit.as_path()));
        assert_eq!(
            std::env::var("WF_RUNTIME_ENV_TEST_EXPLICIT").unwrap(),
            "explicit"
        );

        std::env::set_var(EXPLICIT_PATH_VAR, other.path().join("absent.env"));
        let error = load_from(&search_in(support.path())).unwrap_err();
        assert!(format!("{error:#}").contains(EXPLICIT_PATH_VAR));
    }

    #[test]
    fn support_dir_beats_executable_sidecar() {
        let _lock = lock_env();
        let _key = EnvGuard::hold("WF_RUNTIME_ENV_TEST_ORDER");
        let _explicit = EnvGuard::hold(EXPLICIT_PATH_VAR);
        std::env::remove_var("WF_RUNTIME_ENV_TEST_ORDER");
        std::env::remove_var(EXPLICIT_PATH_VAR);

        let support = tempfile::tempdir().unwrap();
        let exe = tempfile::tempdir().unwrap();
        std::fs::write(
            support.path().join(ENV_FILE_NAME),
            "WF_RUNTIME_ENV_TEST_ORDER=support\n",
        )
        .unwrap();
        std::fs::write(
            exe.path().join(ENV_FILE_NAME),
            "WF_RUNTIME_ENV_TEST_ORDER=sidecar\n",
        )
        .unwrap();

        let report = load_from(&SearchDirs {
            exe_dir: Some(exe.path().to_path_buf()),
            support_dir: Some(support.path().to_path_buf()),
        })
        .unwrap();
        assert_eq!(
            report.path.as_deref(),
            Some(support.path().join(ENV_FILE_NAME).as_path())
        );
        assert_eq!(
            std::env::var("WF_RUNTIME_ENV_TEST_ORDER").unwrap(),
            "support"
        );
    }
}
