// SPDX-License-Identifier: MIT
//! Marketplace catalog: offline-first SQLite mirror, search, sync.
//!
//! ## Storage
//!
//! `~/.impforge/mcp-marketplace.db` (SQLite + WAL).  Schema is identical
//! semantically to the CLI tier (`impforge-cli mcp_marketplace`) but
//! lives in a separate location so installing the CLI alongside the App
//! does not collide.
//!
//! ## Privacy
//!
//! `sync_mirror` does ONE outbound HTTP GET against the public CDN.  No
//! POST, no telemetry, no user data ever leaves.

use crate::error::{AppError, AppResult};
use crate::mcp::types::{Category, CapabilityHint, EnvVarSpec, MarketplaceEntry, Transport};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

/// Default marketplace URL (public CDN).
pub const DEFAULT_MARKETPLACE_URL: &str =
    "https://marketplace.impforge.com/v1/manifests.json";

/// Hard ceiling on a single marketplace JSON download (32 MB).
pub const MAX_MARKETPLACE_BYTES: usize = 32 * 1024 * 1024;

/// HTTP timeout for `sync_mirror` (10s).
pub const MIRROR_TIMEOUT_SECS: u64 = 10;

/// User-Agent for the marketplace fetch.
pub const MARKETPLACE_USER_AGENT: &str = concat!(
    "impforge-app/",
    env!("CARGO_PKG_VERSION"),
    " (+https://impforge.com)"
);

/// Resolve `~/.impforge/` (overridable via `IMPFORGE_APP_HOME` for tests).
pub fn app_home() -> AppResult<PathBuf> {
    if let Ok(custom) = std::env::var("IMPFORGE_APP_HOME") {
        let p = PathBuf::from(custom);
        fs::create_dir_all(&p).map_err(AppError::from)?;
        return Ok(p);
    }
    let base = dirs::home_dir().ok_or_else(|| AppError::Internal("no $HOME".into()))?;
    let p = base.join(".impforge");
    fs::create_dir_all(&p).map_err(AppError::from)?;
    Ok(p)
}

/// Path to the marketplace SQLite mirror.
pub fn marketplace_db_path() -> AppResult<PathBuf> {
    Ok(app_home()?.join("mcp-marketplace.db"))
}

/// One global connection — wrapped in a `Mutex` because rusqlite is
/// `Send` but not `Sync`, and Tauri commands run on a multithreaded
/// runtime.  `pub(crate)` so sibling tests can reset the connection
/// when they swap `IMPFORGE_APP_HOME`.
pub(crate) static MARKETPLACE_CONN: Mutex<Option<Connection>> = Mutex::new(None);

/// Module-wide lock guarding the `IMPFORGE_APP_HOME` env var across
/// every mcp:: test.  Tests share process state, so without this lock
/// they race when run in parallel and produce flaky failures.  Held
/// for the lifetime of one test by the `HomeGuard` helpers.
#[cfg(test)]
pub(crate) static TEST_ENV_LOCK: Mutex<()> = Mutex::new(());

/// Open (or reuse) the marketplace connection.  Idempotent.
pub fn open_db() -> AppResult<()> {
    let mut guard = MARKETPLACE_CONN
        .lock()
        .map_err(|e| AppError::Internal(format!("marketplace mutex poisoned: {e}")))?;
    if guard.is_none() {
        let path = marketplace_db_path()?;
        let conn = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| AppError::Internal(format!("open marketplace db: {e}")))?;
        conn.execute_batch(
            r#"
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;

CREATE TABLE IF NOT EXISTS marketplace (
    id              TEXT PRIMARY KEY,
    payload         TEXT NOT NULL,
    name            TEXT NOT NULL,
    description     TEXT NOT NULL,
    publisher       TEXT NOT NULL,
    category        TEXT NOT NULL,
    install_count   INTEGER NOT NULL DEFAULT 0,
    stars           INTEGER NOT NULL DEFAULT 0,
    verified        INTEGER NOT NULL DEFAULT 0,
    signed          INTEGER NOT NULL DEFAULT 0,
    last_updated    TEXT NOT NULL,
    refreshed_at    TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_marketplace_category
    ON marketplace (category);
CREATE INDEX IF NOT EXISTS idx_marketplace_install_count
    ON marketplace (install_count DESC);
"#,
        )
        .map_err(|e| AppError::Internal(format!("init marketplace schema: {e}")))?;
        *guard = Some(conn);
    }
    Ok(())
}

/// Run a closure with the marketplace DB connection.
pub fn with_db<F, R>(f: F) -> AppResult<R>
where
    F: FnOnce(&Connection) -> AppResult<R>,
{
    open_db()?;
    let guard = MARKETPLACE_CONN
        .lock()
        .map_err(|e| AppError::Internal(format!("marketplace mutex poisoned: {e}")))?;
    let conn = guard
        .as_ref()
        .ok_or_else(|| AppError::Internal("marketplace conn missing after open".into()))?;
    f(conn)
}

/// Query filters used by the BrowseView.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct BrowseFilters {
    pub search: Option<String>,
    pub category: Option<String>,
    pub limit: Option<u32>,
    pub verified_only: bool,
    pub signed_only: bool,
    pub no_oauth: bool,
}

/// Browse the cached marketplace.  Returns up to `limit` entries
/// (default 100), sorted by install_count desc.
pub fn browse(filters: &BrowseFilters) -> AppResult<Vec<MarketplaceEntry>> {
    ensure_seeded()?;
    with_db(|conn| {
        let mut sql = String::from("SELECT payload FROM marketplace WHERE 1=1");
        let mut bindings: Vec<String> = Vec::new();
        if let Some(cat) = filters.category.as_ref() {
            sql.push_str(" AND category = ?");
            bindings.push(cat.to_lowercase());
        }
        if let Some(q) = filters.search.as_ref().filter(|s| !s.is_empty()) {
            let pattern = format!("%{}%", q.to_lowercase());
            sql.push_str(
                " AND (LOWER(name) LIKE ? OR LOWER(description) LIKE ? OR LOWER(publisher) LIKE ?)",
            );
            bindings.push(pattern.clone());
            bindings.push(pattern.clone());
            bindings.push(pattern);
        }
        if filters.verified_only {
            sql.push_str(" AND verified = 1");
        }
        if filters.signed_only {
            sql.push_str(" AND signed = 1");
        }
        sql.push_str(" ORDER BY install_count DESC, last_updated DESC LIMIT ?");
        let limit = filters.limit.unwrap_or(100).min(500);
        bindings.push(limit.to_string());
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| AppError::Internal(format!("prepare browse query: {e}")))?;
        let mapped: rusqlite::Result<Vec<String>> = stmt
            .query_map(rusqlite::params_from_iter(bindings.iter()), |row| {
                row.get(0)
            })
            .map_err(|e| AppError::Internal(format!("query marketplace: {e}")))?
            .collect();
        let payloads = mapped
            .map_err(|e| AppError::Internal(format!("collect marketplace rows: {e}")))?;
        let mut out = Vec::with_capacity(payloads.len());
        for p in payloads {
            let entry: MarketplaceEntry = serde_json::from_str(&p)
                .map_err(|e| AppError::Internal(format!("decode entry payload: {e}")))?;
            if filters.no_oauth && entry.oauth {
                continue;
            }
            out.push(entry);
        }
        Ok(out)
    })
}

/// Fetch a single entry by id.  Used by `DetailView`.
pub fn get(id: &str) -> AppResult<MarketplaceEntry> {
    with_db(|conn| {
        let mut stmt = conn
            .prepare("SELECT payload FROM marketplace WHERE id = ?1")
            .map_err(|e| AppError::Internal(format!("prepare get: {e}")))?;
        let payload: String = stmt
            .query_row(params![id], |row| row.get(0))
            .map_err(|_| AppError::NotFound(format!("server '{id}' not in marketplace cache")))?;
        let entry: MarketplaceEntry = serde_json::from_str(&payload)
            .map_err(|e| AppError::Internal(format!("decode entry: {e}")))?;
        Ok(entry)
    })
}

/// Insert (or replace) one entry.
pub fn upsert(entry: &MarketplaceEntry) -> AppResult<()> {
    with_db(|conn| {
        let payload = serde_json::to_string(entry)
            .map_err(|e| AppError::Internal(format!("encode entry: {e}")))?;
        conn.execute(
            r#"INSERT INTO marketplace (id, payload, name, description, publisher,
                                         category, install_count, stars, verified, signed,
                                         last_updated, refreshed_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
               ON CONFLICT(id) DO UPDATE SET
                 payload = excluded.payload,
                 name = excluded.name,
                 description = excluded.description,
                 publisher = excluded.publisher,
                 category = excluded.category,
                 install_count = excluded.install_count,
                 stars = excluded.stars,
                 verified = excluded.verified,
                 signed = excluded.signed,
                 last_updated = excluded.last_updated,
                 refreshed_at = excluded.refreshed_at"#,
            params![
                entry.id,
                payload,
                entry.name,
                entry.description,
                entry.publisher,
                entry.category.as_str(),
                entry.install_count as i64,
                entry.stars as i64,
                entry.verified as i32,
                entry.signed as i32,
                entry.last_updated.to_rfc3339(),
                Utc::now().to_rfc3339(),
            ],
        )
        .map_err(|e| AppError::Internal(format!("upsert entry: {e}")))?;
        Ok(())
    })
}

/// Build the curated 8-server seed (matches `impforge-cli`).
pub fn seed_entries() -> Vec<MarketplaceEntry> {
    let now = Utc::now();
    let env_token = |name: &str, desc: &str, secret: bool| EnvVarSpec {
        name: name.into(),
        description: desc.into(),
        secret,
        required: true,
    };
    vec![
        MarketplaceEntry {
            id: "filesystem".into(),
            name: "Filesystem".into(),
            publisher: "modelcontextprotocol".into(),
            version: "0.6.0".into(),
            description: "Read & write local files inside the user's allow-listed paths."
                .into(),
            category: Category::Files,
            tags: vec!["files".into(), "local".into()],
            transport: Transport::Stdio,
            command: Some("npx".into()),
            args: vec!["-y".into(), "@modelcontextprotocol/server-filesystem".into()],
            env_schema: vec![],
            homepage: Some("https://github.com/modelcontextprotocol/servers".into()),
            license: Some("MIT".into()),
            install_count: 142_000,
            stars: 5,
            verified: true,
            signed: true,
            oauth: false,
            last_updated: now,
            capability_hint: CapabilityHint {
                tools: 6,
                resources: 1,
                prompts: 0,
            },
        },
        MarketplaceEntry {
            id: "github".into(),
            name: "GitHub".into(),
            publisher: "modelcontextprotocol".into(),
            version: "0.7.1".into(),
            description: "Issues, PRs, commits, and code search across repositories.".into(),
            category: Category::Code,
            tags: vec!["github".into(), "code".into(), "oauth".into()],
            transport: Transport::Stdio,
            command: Some("npx".into()),
            args: vec!["-y".into(), "@modelcontextprotocol/server-github".into()],
            env_schema: vec![env_token(
                "GITHUB_PERSONAL_ACCESS_TOKEN",
                "GitHub PAT with repo + read:org scope.",
                true,
            )],
            homepage: Some("https://github.com/modelcontextprotocol/servers".into()),
            license: Some("MIT".into()),
            install_count: 96_000,
            stars: 5,
            verified: true,
            signed: true,
            oauth: true,
            last_updated: now,
            capability_hint: CapabilityHint {
                tools: 24,
                resources: 0,
                prompts: 0,
            },
        },
        MarketplaceEntry {
            id: "brave-search".into(),
            name: "Brave Search".into(),
            publisher: "modelcontextprotocol".into(),
            version: "0.5.0".into(),
            description: "Live web search via the Brave Search API (BYOK).".into(),
            category: Category::Web,
            tags: vec!["search".into(), "web".into()],
            transport: Transport::Stdio,
            command: Some("npx".into()),
            args: vec!["-y".into(), "@modelcontextprotocol/server-brave-search".into()],
            env_schema: vec![env_token("BRAVE_API_KEY", "Brave Search API key.", true)],
            homepage: Some("https://github.com/modelcontextprotocol/servers".into()),
            license: Some("MIT".into()),
            install_count: 71_000,
            stars: 4,
            verified: true,
            signed: false,
            oauth: false,
            last_updated: now,
            capability_hint: CapabilityHint {
                tools: 2,
                resources: 0,
                prompts: 0,
            },
        },
        MarketplaceEntry {
            id: "memory".into(),
            name: "Memory".into(),
            publisher: "modelcontextprotocol".into(),
            version: "0.4.2".into(),
            description: "Long-term key/value memory backed by a local store.".into(),
            category: Category::Data,
            tags: vec!["memory".into(), "local".into()],
            transport: Transport::Stdio,
            command: Some("npx".into()),
            args: vec!["-y".into(), "@modelcontextprotocol/server-memory".into()],
            env_schema: vec![],
            homepage: Some("https://github.com/modelcontextprotocol/servers".into()),
            license: Some("MIT".into()),
            install_count: 58_000,
            stars: 4,
            verified: true,
            signed: true,
            oauth: false,
            last_updated: now,
            capability_hint: CapabilityHint {
                tools: 5,
                resources: 0,
                prompts: 0,
            },
        },
        MarketplaceEntry {
            id: "fetch".into(),
            name: "Fetch".into(),
            publisher: "modelcontextprotocol".into(),
            version: "0.3.0".into(),
            description: "HTTP fetch with markdown / text / JSON parsing.".into(),
            category: Category::Web,
            tags: vec!["http".into(), "web".into()],
            transport: Transport::Stdio,
            command: Some("uvx".into()),
            args: vec!["mcp-server-fetch".into()],
            env_schema: vec![],
            homepage: Some("https://github.com/modelcontextprotocol/servers".into()),
            license: Some("MIT".into()),
            install_count: 45_000,
            stars: 4,
            verified: true,
            signed: false,
            oauth: false,
            last_updated: now,
            capability_hint: CapabilityHint {
                tools: 1,
                resources: 0,
                prompts: 0,
            },
        },
        MarketplaceEntry {
            id: "git".into(),
            name: "Git".into(),
            publisher: "modelcontextprotocol".into(),
            version: "0.4.0".into(),
            description: "Run git operations on a local repository.".into(),
            category: Category::Code,
            tags: vec!["git".into(), "code".into()],
            transport: Transport::Stdio,
            command: Some("uvx".into()),
            args: vec!["mcp-server-git".into()],
            env_schema: vec![],
            homepage: Some("https://github.com/modelcontextprotocol/servers".into()),
            license: Some("MIT".into()),
            install_count: 38_000,
            stars: 4,
            verified: true,
            signed: true,
            oauth: false,
            last_updated: now,
            capability_hint: CapabilityHint {
                tools: 9,
                resources: 0,
                prompts: 0,
            },
        },
        MarketplaceEntry {
            id: "postgres".into(),
            name: "Postgres".into(),
            publisher: "modelcontextprotocol".into(),
            version: "0.3.0".into(),
            description: "Read-only Postgres database access via connection string."
                .into(),
            category: Category::Data,
            tags: vec!["postgres".into(), "sql".into(), "data".into()],
            transport: Transport::Stdio,
            command: Some("npx".into()),
            args: vec!["-y".into(), "@modelcontextprotocol/server-postgres".into()],
            env_schema: vec![env_token(
                "POSTGRES_CONNECTION_STRING",
                "Read-only Postgres connection string.",
                true,
            )],
            homepage: Some("https://github.com/modelcontextprotocol/servers".into()),
            license: Some("MIT".into()),
            install_count: 24_000,
            stars: 4,
            verified: true,
            signed: true,
            oauth: false,
            last_updated: now,
            capability_hint: CapabilityHint {
                tools: 1,
                resources: 1,
                prompts: 0,
            },
        },
        MarketplaceEntry {
            id: "slack".into(),
            name: "Slack".into(),
            publisher: "modelcontextprotocol".into(),
            version: "0.5.0".into(),
            description: "Send + read Slack messages via OAuth.".into(),
            category: Category::Comms,
            tags: vec!["slack".into(), "chat".into(), "oauth".into()],
            transport: Transport::Stdio,
            command: Some("npx".into()),
            args: vec!["-y".into(), "@modelcontextprotocol/server-slack".into()],
            env_schema: vec![],
            homepage: Some("https://github.com/modelcontextprotocol/servers".into()),
            license: Some("MIT".into()),
            install_count: 19_000,
            stars: 4,
            verified: true,
            signed: false,
            oauth: true,
            last_updated: now,
            capability_hint: CapabilityHint {
                tools: 8,
                resources: 0,
                prompts: 0,
            },
        },
    ]
}

/// Seed an empty DB with the bundled curated set.  Idempotent.
pub fn ensure_seeded() -> AppResult<usize> {
    let count: i64 = with_db(|conn| {
        conn.query_row("SELECT COUNT(*) FROM marketplace", [], |row| row.get(0))
            .map_err(|e| AppError::Internal(format!("count rows: {e}")))
    })?;
    if count > 0 {
        return Ok(0);
    }
    let entries = seed_entries();
    let inserted = entries.len();
    for e in &entries {
        upsert(e)?;
    }
    Ok(inserted)
}

/// Sync the marketplace from the public CDN.
pub async fn sync_mirror(url: Option<String>) -> AppResult<usize> {
    let target = url.unwrap_or_else(|| DEFAULT_MARKETPLACE_URL.to_string());
    let client = reqwest::Client::builder()
        .user_agent(MARKETPLACE_USER_AGENT)
        .timeout(std::time::Duration::from_secs(MIRROR_TIMEOUT_SECS))
        .build()
        .map_err(|e| AppError::Internal(format!("build http client: {e}")))?;
    let resp = client
        .get(&target)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("marketplace fetch: {e}")))?;
    if !resp.status().is_success() {
        return Err(AppError::Internal(format!(
            "marketplace returned HTTP {}",
            resp.status()
        )));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| AppError::Internal(format!("read marketplace bytes: {e}")))?;
    if bytes.len() > MAX_MARKETPLACE_BYTES {
        return Err(AppError::Internal(format!(
            "marketplace payload {} bytes exceeds {} byte cap",
            bytes.len(),
            MAX_MARKETPLACE_BYTES
        )));
    }
    let entries: Vec<MarketplaceEntry> = serde_json::from_slice(&bytes)
        .map_err(|e| AppError::Internal(format!("parse marketplace JSON: {e}")))?;
    let count = entries.len();
    for e in entries {
        upsert(&e)?;
    }
    Ok(count)
}

/// Status of the offline mirror — drives the Settings header indicator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineStatus {
    pub entries_cached: u64,
    pub last_refresh: Option<DateTime<Utc>>,
    pub db_path: PathBuf,
    pub has_network_baseline: bool,
}

/// Quick offline-mirror status report.
pub fn offline_status() -> AppResult<OfflineStatus> {
    open_db()?;
    let path = marketplace_db_path()?;
    with_db(|conn| {
        let entries_cached: i64 = conn
            .query_row("SELECT COUNT(*) FROM marketplace", [], |row| row.get(0))
            .map_err(|e| AppError::Internal(format!("count rows: {e}")))?;
        let last_refresh: Option<String> = conn
            .query_row(
                "SELECT MAX(refreshed_at) FROM marketplace",
                [],
                |row| row.get::<_, Option<String>>(0),
            )
            .map_err(|e| AppError::Internal(format!("max refreshed_at: {e}")))?;
        let last_refresh = last_refresh
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&Utc));
        Ok(OfflineStatus {
            entries_cached: entries_cached.max(0) as u64,
            last_refresh,
            db_path: path.clone(),
            has_network_baseline: entries_cached > 0,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Resets the global connection so each test gets a fresh DB in its
    /// own tempdir.  Each test must hold a `HomeGuard` for the duration.
    fn reset_conn() {
        let mut g = MARKETPLACE_CONN.lock().expect("lock mutex");
        *g = None;
    }

    struct HomeGuard {
        _tmp: TempDir,
        prev: Option<String>,
        _env_lock: std::sync::MutexGuard<'static, ()>,
    }

    fn set_home() -> HomeGuard {
        let lock = TEST_ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = tempfile::tempdir().expect("tempdir");
        let prev = std::env::var("IMPFORGE_APP_HOME").ok();
        std::env::set_var("IMPFORGE_APP_HOME", tmp.path());
        reset_conn();
        HomeGuard {
            _tmp: tmp,
            prev,
            _env_lock: lock,
        }
    }

    impl Drop for HomeGuard {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => std::env::set_var("IMPFORGE_APP_HOME", v),
                None => std::env::remove_var("IMPFORGE_APP_HOME"),
            }
            reset_conn();
        }
    }

    #[test]
    fn ensure_seeded_then_idempotent() {
        let _g = set_home();
        let first = ensure_seeded().expect("first seed");
        assert!(first >= 8);
        let second = ensure_seeded().expect("second seed");
        assert_eq!(second, 0, "ensure_seeded must be idempotent");
    }

    #[test]
    fn browse_filters_by_category() {
        let _g = set_home();
        ensure_seeded().expect("seed");
        let code = browse(&BrowseFilters {
            category: Some("code".into()),
            ..Default::default()
        })
        .expect("browse code");
        assert!(!code.is_empty());
        assert!(code.iter().all(|e| e.category == Category::Code));
    }

    #[test]
    fn browse_search_substring_matches_name_and_description() {
        let _g = set_home();
        ensure_seeded().expect("seed");
        let github = browse(&BrowseFilters {
            search: Some("github".into()),
            ..Default::default()
        })
        .expect("browse github");
        assert!(github.iter().any(|e| e.id == "github"));
        let memory = browse(&BrowseFilters {
            search: Some("memory".into()),
            ..Default::default()
        })
        .expect("browse memory");
        assert!(memory.iter().any(|e| e.id == "memory"));
    }

    #[test]
    fn browse_no_oauth_filter_excludes_oauth_servers() {
        let _g = set_home();
        ensure_seeded().expect("seed");
        let no_oauth = browse(&BrowseFilters {
            no_oauth: true,
            ..Default::default()
        })
        .expect("browse no oauth");
        assert!(no_oauth.iter().all(|e| !e.oauth), "no-oauth filter must exclude oauth servers");
    }

    #[test]
    fn browse_verified_filter_only_returns_verified() {
        let _g = set_home();
        ensure_seeded().expect("seed");
        let verified = browse(&BrowseFilters {
            verified_only: true,
            ..Default::default()
        })
        .expect("browse verified");
        assert!(verified.iter().all(|e| e.verified));
    }

    #[test]
    fn get_unknown_id_returns_not_found() {
        let _g = set_home();
        ensure_seeded().expect("seed");
        let err = get("does-not-exist").expect_err("must fail");
        assert!(matches!(err, AppError::NotFound(_)));
    }

    #[test]
    fn offline_status_reports_seeded_count() {
        let _g = set_home();
        ensure_seeded().expect("seed");
        let s = offline_status().expect("status");
        assert!(s.entries_cached >= 8);
        assert!(s.has_network_baseline);
    }
}
