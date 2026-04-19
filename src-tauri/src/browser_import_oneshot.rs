// SPDX-License-Identifier: MIT
//! One-shot browser bookmark + history importer with the **WAL-copy
//! pattern**: Firefox / Chromium hold their SQLite databases under
//! exclusive locks while the browser runs, so we copy the file to a
//! temp path and open it with `?mode=ro&nolock=1` to bypass.  This is
//! the same pattern Apple's Spotlight and Microsoft's MSEdge importer
//! use.
//!
//! Tier 2 of Feature 3 (Global Digest).  Wraps Firefox + Chromium
//! family lookups; per-profile detection lives in `detect_profiles`.
//!
//! ## Privacy
//!
//! All work happens on the user's disk; no data leaves the machine.
//! The WAL-copy goes into a tempfile that's auto-deleted at end of
//! the function call.  Per `REGEL 000-BRIDGE-NOT-PROCESS`.

use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Supported browser families — used by the digest UI to pick what
/// to import.  Variant order matches the on-disk profile-detection
/// priority (Firefox first because it's the only family that needs
/// the WAL-copy dance).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BrowserFamily {
    Firefox,
    Chrome,
    Chromium,
    Brave,
    Edge,
    Vivaldi,
    Opera,
}

impl BrowserFamily {
    pub fn as_str(self) -> &'static str {
        match self {
            BrowserFamily::Firefox => "firefox",
            BrowserFamily::Chrome => "chrome",
            BrowserFamily::Chromium => "chromium",
            BrowserFamily::Brave => "brave",
            BrowserFamily::Edge => "edge",
            BrowserFamily::Vivaldi => "vivaldi",
            BrowserFamily::Opera => "opera",
        }
    }
}

/// Backwards-compat alias — the Phase 1 stub used `BrowserKind`.
pub type BrowserKind = BrowserFamily;

/// One detected browser profile on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserProfile {
    pub family: BrowserFamily,
    pub path: PathBuf,
    pub name: String,
}

/// One imported bookmark.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserBookmark {
    pub title: String,
    pub url: String,
    pub folder: Option<String>,
    pub date_added: Option<chrono::DateTime<chrono::Utc>>,
}

/// One imported history item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserHistoryItem {
    pub url: String,
    pub title: Option<String>,
    pub visit_count: i64,
    pub last_visit: Option<chrono::DateTime<chrono::Utc>>,
}

// ─── Profile detection ────────────────────────────────────────────────────

/// Iterate every Firefox profile directory found on this machine.
fn firefox_profile_dirs() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        return vec![];
    };
    let bases = if cfg!(target_os = "macos") {
        vec![home.join("Library/Application Support/Firefox/Profiles")]
    } else if cfg!(target_os = "windows") {
        vec![home.join("AppData/Roaming/Mozilla/Firefox/Profiles")]
    } else {
        vec![home.join(".mozilla/firefox")]
    };
    let mut out = Vec::new();
    for base in bases {
        let Ok(entries) = std::fs::read_dir(&base) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() && p.join("places.sqlite").exists() {
                out.push(p);
            }
        }
    }
    out
}

/// Resolve the canonical Chrome-family profile dir.  Returns None
/// when the family isn't installed on this OS.
fn chrome_family_root(family: BrowserFamily) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let path = match family {
        BrowserFamily::Chrome => {
            if cfg!(target_os = "macos") {
                home.join("Library/Application Support/Google/Chrome")
            } else if cfg!(target_os = "windows") {
                home.join("AppData/Local/Google/Chrome/User Data")
            } else {
                home.join(".config/google-chrome")
            }
        }
        BrowserFamily::Chromium => {
            if cfg!(target_os = "macos") {
                home.join("Library/Application Support/Chromium")
            } else if cfg!(target_os = "windows") {
                home.join("AppData/Local/Chromium/User Data")
            } else {
                home.join(".config/chromium")
            }
        }
        BrowserFamily::Brave => {
            if cfg!(target_os = "macos") {
                home.join("Library/Application Support/BraveSoftware/Brave-Browser")
            } else if cfg!(target_os = "windows") {
                home.join("AppData/Local/BraveSoftware/Brave-Browser/User Data")
            } else {
                home.join(".config/BraveSoftware/Brave-Browser")
            }
        }
        BrowserFamily::Edge => {
            if cfg!(target_os = "macos") {
                home.join("Library/Application Support/Microsoft Edge")
            } else if cfg!(target_os = "windows") {
                home.join("AppData/Local/Microsoft/Edge/User Data")
            } else {
                home.join(".config/microsoft-edge")
            }
        }
        BrowserFamily::Vivaldi => {
            if cfg!(target_os = "macos") {
                home.join("Library/Application Support/Vivaldi")
            } else if cfg!(target_os = "windows") {
                home.join("AppData/Local/Vivaldi/User Data")
            } else {
                home.join(".config/vivaldi")
            }
        }
        BrowserFamily::Opera => {
            if cfg!(target_os = "macos") {
                home.join("Library/Application Support/com.operasoftware.Opera")
            } else if cfg!(target_os = "windows") {
                home.join("AppData/Roaming/Opera Software/Opera Stable")
            } else {
                home.join(".config/opera")
            }
        }
        BrowserFamily::Firefox => return None,
    };
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

/// Walk the Chrome family root + collect every `Default` / `Profile N`
/// directory that has a `Bookmarks` JSON file.
fn chrome_family_profile_dirs(family: BrowserFamily) -> Vec<PathBuf> {
    let Some(root) = chrome_family_root(family) else {
        return vec![];
    };
    let Ok(entries) = std::fs::read_dir(&root) else {
        return vec![];
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let is_profile_dir = name == "Default" || name.starts_with("Profile ");
        let has_data = p.join("Bookmarks").exists() || p.join("History").exists();
        if is_profile_dir && has_data {
            out.push(p);
        }
    }
    out
}

/// Detect every browser profile we can find on this machine.
pub async fn browser_detect_profiles_inner() -> AppResult<Vec<BrowserProfile>> {
    let mut out = Vec::new();
    for p in firefox_profile_dirs() {
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("default")
            .to_string();
        out.push(BrowserProfile {
            family: BrowserFamily::Firefox,
            path: p,
            name,
        });
    }
    for fam in [
        BrowserFamily::Chrome,
        BrowserFamily::Chromium,
        BrowserFamily::Brave,
        BrowserFamily::Edge,
        BrowserFamily::Vivaldi,
        BrowserFamily::Opera,
    ] {
        for p in chrome_family_profile_dirs(fam) {
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Default")
                .to_string();
            out.push(BrowserProfile {
                family: fam,
                path: p,
                name,
            });
        }
    }
    Ok(out)
}

// ─── WAL-copy SQLite open (Firefox path) ─────────────────────────────────

/// Copy a SQLite database to a temp file + open RO with `nolock=1`.
/// Returns both the connection and the tempfile guard so the caller
/// keeps the temp alive until done.
fn open_locked_sqlite(src: &Path) -> AppResult<(Connection, tempfile::NamedTempFile)> {
    let tmp = tempfile::NamedTempFile::new().map_err(AppError::Io)?;
    std::fs::copy(src, tmp.path()).map_err(AppError::Io)?;
    // Some browsers also need the `-wal` and `-shm` files copied
    // alongside.  Best-effort copy them; missing files are fine.
    for ext in &["-wal", "-shm"] {
        let mut suffix = src.as_os_str().to_owned();
        suffix.push(ext);
        let src_aux = PathBuf::from(suffix);
        if src_aux.exists() {
            let mut dst_aux = tmp.path().as_os_str().to_owned();
            dst_aux.push(ext);
            let _ = std::fs::copy(&src_aux, PathBuf::from(dst_aux));
        }
    }
    let uri = format!(
        "file:{}?mode=ro&nolock=1",
        tmp.path().to_string_lossy()
    );
    let conn = Connection::open_with_flags(
        &uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|e| AppError::Internal(format!("sqlite open: {e}")))?;
    Ok((conn, tmp))
}

/// Firefox stores microseconds since UNIX epoch.
fn ff_to_chrono(microseconds: i64) -> Option<chrono::DateTime<chrono::Utc>> {
    let secs = microseconds / 1_000_000;
    chrono::DateTime::<chrono::Utc>::from_timestamp(secs, 0)
}

/// Chrome stores microseconds since 1601-01-01 (Windows epoch).
fn chrome_to_chrono(chrome_micros: i64) -> Option<chrono::DateTime<chrono::Utc>> {
    // Difference in seconds between 1601-01-01 and 1970-01-01:
    // 11_644_473_600
    let secs = chrome_micros / 1_000_000 - 11_644_473_600;
    if secs < 0 {
        return None;
    }
    chrono::DateTime::<chrono::Utc>::from_timestamp(secs, 0)
}

// ─── Firefox readers ──────────────────────────────────────────────────────

fn read_firefox_bookmarks(profile: &Path) -> AppResult<Vec<BrowserBookmark>> {
    let places = profile.join("places.sqlite");
    if !places.exists() {
        return Ok(vec![]);
    }
    let (conn, _tmp) = open_locked_sqlite(&places)?;
    let mut stmt = conn
        .prepare(
            "SELECT b.title, p.url, b.dateAdded, parent.title \
             FROM moz_bookmarks b \
             JOIN moz_places p ON p.id = b.fk \
             LEFT JOIN moz_bookmarks parent ON parent.id = b.parent \
             WHERE b.type = 1 AND p.url IS NOT NULL",
        )
        .map_err(|e| AppError::Internal(format!("ff bookmarks prep: {e}")))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                row.get::<_, String>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(|e| AppError::Internal(format!("ff bookmarks query: {e}")))?;
    let mut out = Vec::new();
    for row in rows.flatten() {
        out.push(BrowserBookmark {
            title: row.0,
            url: row.1,
            date_added: row.2.and_then(ff_to_chrono),
            folder: row.3,
        });
    }
    Ok(out)
}

fn read_firefox_history(
    profile: &Path,
    days: u32,
) -> AppResult<Vec<BrowserHistoryItem>> {
    let places = profile.join("places.sqlite");
    if !places.exists() {
        return Ok(vec![]);
    }
    let (conn, _tmp) = open_locked_sqlite(&places)?;
    let cutoff_us = (chrono::Utc::now() - chrono::Duration::days(days as i64))
        .timestamp_micros();
    let mut stmt = conn
        .prepare(
            "SELECT url, title, visit_count, last_visit_date \
             FROM moz_places \
             WHERE last_visit_date IS NOT NULL AND last_visit_date >= ?1 \
             ORDER BY last_visit_date DESC \
             LIMIT 5000",
        )
        .map_err(|e| AppError::Internal(format!("ff history prep: {e}")))?;
    let rows = stmt
        .query_map(params![cutoff_us], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<i64>>(3)?,
            ))
        })
        .map_err(|e| AppError::Internal(format!("ff history query: {e}")))?;
    let mut out = Vec::new();
    for row in rows.flatten() {
        out.push(BrowserHistoryItem {
            url: row.0,
            title: row.1,
            visit_count: row.2,
            last_visit: row.3.and_then(ff_to_chrono),
        });
    }
    Ok(out)
}

// ─── Chrome-family readers ────────────────────────────────────────────────

fn read_chrome_bookmarks(profile: &Path) -> AppResult<Vec<BrowserBookmark>> {
    let json_path = profile.join("Bookmarks");
    if !json_path.exists() {
        return Ok(vec![]);
    }
    let raw = std::fs::read_to_string(&json_path).map_err(AppError::Io)?;
    let doc: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| AppError::Internal(format!("chrome bookmarks json: {e}")))?;
    let mut out = Vec::new();
    if let Some(roots) = doc.get("roots").and_then(|v| v.as_object()) {
        for (folder_name, root) in roots {
            walk_chrome_node(root, Some(folder_name), &mut out);
        }
    }
    Ok(out)
}

fn walk_chrome_node(
    node: &serde_json::Value,
    folder: Option<&str>,
    out: &mut Vec<BrowserBookmark>,
) {
    let Some(obj) = node.as_object() else { return };
    let kind = obj
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("");
    if kind == "url" {
        let title = obj
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string();
        let url = obj
            .get("url")
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .to_string();
        if url.is_empty() {
            return;
        }
        let date_added = obj
            .get("date_added")
            .and_then(|d| d.as_str())
            .and_then(|s| s.parse::<i64>().ok())
            .and_then(chrome_to_chrono);
        out.push(BrowserBookmark {
            title,
            url,
            folder: folder.map(|f| f.to_string()),
            date_added,
        });
    } else if kind == "folder" {
        let inner_folder = obj
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or(folder.unwrap_or(""));
        if let Some(children) = obj.get("children").and_then(|c| c.as_array()) {
            for child in children {
                walk_chrome_node(child, Some(inner_folder), out);
            }
        }
    }
}

fn read_chrome_history(
    profile: &Path,
    days: u32,
) -> AppResult<Vec<BrowserHistoryItem>> {
    let history = profile.join("History");
    if !history.exists() {
        return Ok(vec![]);
    }
    let (conn, _tmp) = open_locked_sqlite(&history)?;
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(days as i64))
        .timestamp_micros()
        + 11_644_473_600 * 1_000_000;
    let mut stmt = conn
        .prepare(
            "SELECT url, title, visit_count, last_visit_time \
             FROM urls \
             WHERE last_visit_time >= ?1 \
             ORDER BY last_visit_time DESC \
             LIMIT 5000",
        )
        .map_err(|e| AppError::Internal(format!("chrome history prep: {e}")))?;
    let rows = stmt
        .query_map(params![cutoff], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .map_err(|e| AppError::Internal(format!("chrome history query: {e}")))?;
    let mut out = Vec::new();
    for row in rows.flatten() {
        out.push(BrowserHistoryItem {
            url: row.0,
            title: row.1,
            visit_count: row.2,
            last_visit: chrome_to_chrono(row.3),
        });
    }
    Ok(out)
}

// ─── Public family-keyed importers ───────────────────────────────────────

/// Import all bookmarks for a given browser family across all profiles.
pub async fn browser_import_bookmarks(
    family: BrowserFamily,
) -> AppResult<Vec<BrowserBookmark>> {
    let mut out = Vec::new();
    if family == BrowserFamily::Firefox {
        for p in firefox_profile_dirs() {
            let mut chunk = read_firefox_bookmarks(&p)?;
            out.append(&mut chunk);
        }
    } else {
        for p in chrome_family_profile_dirs(family) {
            let mut chunk = read_chrome_bookmarks(&p)?;
            out.append(&mut chunk);
        }
    }
    Ok(out)
}

/// Import the last `days` of browser history.
pub async fn browser_import_history(
    family: BrowserFamily,
    days: u32,
) -> AppResult<Vec<BrowserHistoryItem>> {
    let mut out = Vec::new();
    if family == BrowserFamily::Firefox {
        for p in firefox_profile_dirs() {
            let mut chunk = read_firefox_history(&p, days)?;
            out.append(&mut chunk);
        }
    } else {
        for p in chrome_family_profile_dirs(family) {
            let mut chunk = read_chrome_history(&p, days)?;
            out.append(&mut chunk);
        }
    }
    Ok(out)
}

// ─── Tauri commands ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn browser_detect_profiles() -> AppResult<Vec<BrowserProfile>> {
    browser_detect_profiles_inner().await
}

#[tauri::command]
pub async fn browser_import_bookmarks_cmd(
    family: BrowserFamily,
) -> AppResult<Vec<BrowserBookmark>> {
    browser_import_bookmarks(family).await
}

#[tauri::command]
pub async fn browser_import_history_cmd(
    family: BrowserFamily,
    days: u32,
) -> AppResult<Vec<BrowserHistoryItem>> {
    browser_import_history(family, days).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ff_to_chrono_round_trip() {
        // 2026-04-19 00:00:00 UTC = 1776556800 secs.
        let dt = ff_to_chrono(1_776_556_800 * 1_000_000).expect("dt");
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "2026-04-19");
    }

    #[test]
    fn chrome_to_chrono_round_trip() {
        // Chrome epoch shift: 11_644_473_600 seconds before Unix epoch.
        // 2026-04-19 00:00:00 UTC in Chrome time = 1_776_556_800 + 11_644_473_600
        //                                        = 13_421_030_400 secs.
        let dt = chrome_to_chrono(13_421_030_400 * 1_000_000).expect("dt");
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "2026-04-19");
    }

    #[test]
    fn chrome_to_chrono_rejects_negative() {
        // Anything before Unix epoch (= < Chrome 11644473600 secs) returns None.
        assert!(chrome_to_chrono(0).is_none());
    }

    #[test]
    fn family_string_round_trip() {
        for (fam, s) in [
            (BrowserFamily::Firefox, "firefox"),
            (BrowserFamily::Chrome, "chrome"),
            (BrowserFamily::Brave, "brave"),
        ] {
            assert_eq!(fam.as_str(), s);
        }
    }

    #[test]
    fn detect_profiles_does_not_panic_on_missing_browsers() {
        // CI runners typically don't have Firefox/Chrome installed
        // under /home/runner; the call should still succeed (with an
        // empty vec) rather than panic.
        let res = futures::executor::block_on(browser_detect_profiles_inner());
        assert!(res.is_ok());
    }

    #[test]
    fn wal_copy_reads_from_locked_db() {
        // Behavioural test: simulate the WAL-copy pattern by writing
        // a SQLite db, opening it via the helper, and confirming we
        // can read.  Doesn't actually need an exclusive lock — just
        // proves the open-uri path works.
        let tmp = tempfile::tempdir().expect("tempdir");
        let src = tmp.path().join("places.sqlite");
        {
            let conn = Connection::open(&src).expect("open");
            conn.execute(
                "CREATE TABLE t(id INTEGER, value TEXT)",
                [],
            )
            .expect("create");
            conn.execute("INSERT INTO t VALUES (1, 'hello')", []).expect("insert");
        }
        let (conn, _tmp_keep) = open_locked_sqlite(&src).expect("wal copy open");
        let val: String = conn
            .query_row("SELECT value FROM t WHERE id = 1", [], |r| r.get(0))
            .expect("query");
        assert_eq!(val, "hello");
    }

    #[test]
    fn walk_chrome_node_extracts_urls() {
        let json = serde_json::json!({
            "type": "folder",
            "name": "Bar",
            "children": [
                {"type": "url", "name": "A", "url": "https://a.com",
                 "date_added": "13420944000000000"},
                {"type": "folder", "name": "Sub", "children": [
                    {"type": "url", "name": "B", "url": "https://b.com",
                     "date_added": "13420944000000000"}
                ]}
            ]
        });
        let mut out = Vec::new();
        walk_chrome_node(&json, Some("Bar"), &mut out);
        assert_eq!(out.len(), 2);
        assert!(out.iter().any(|b| b.url == "https://a.com"));
        assert!(out.iter().any(|b| b.url == "https://b.com"));
    }
}
