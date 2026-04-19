// SPDX-License-Identifier: MIT
//! HTTP feed pull + parse + entry-to-text + persist pipeline.
//!
//! This is the only sub-module that performs network I/O.  Conditional
//! GET (`If-None-Match` / `If-Modified-Since`) saves bandwidth on
//! unchanged feeds; PII scrubbing happens before any text reaches the
//! shared knowledge DB.

use crate::error::{AppError, AppResult};
use crate::knowledge_lite;

use super::helpers::{cache_dir, sanitize_for_path, scrub_for_ingest};
use super::state::append_history;
use super::types::{
    DigestEntry, FeedPullOutcome, FEED_USER_AGENT, MAX_FEED_BYTES,
};

/// Pull a single feed.  Honours `If-None-Match` / `If-Modified-Since`
/// for conditional GET — feeds that haven't changed return 304 with
/// no body, saving bytes + parser time.
pub async fn pull_feed(
    client: &reqwest::Client,
    url: &str,
    etag: Option<&str>,
    last_modified: Option<&str>,
) -> AppResult<FeedPullOutcome> {
    let mut req = client.get(url).header("User-Agent", FEED_USER_AGENT);
    if let Some(e) = etag {
        req = req.header("If-None-Match", e);
    }
    if let Some(lm) = last_modified {
        req = req.header("If-Modified-Since", lm);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("fetch {url}: {e}")))?;
    if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(FeedPullOutcome {
            not_modified: true,
            ..Default::default()
        });
    }
    if !resp.status().is_success() {
        return Err(AppError::Internal(format!(
            "feed {url}: HTTP {}",
            resp.status()
        )));
    }
    let etag_out = resp
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let lm_out = resp
        .headers()
        .get(reqwest::header::LAST_MODIFIED)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| AppError::Internal(format!("read body {url}: {e}")))?;
    if bytes.len() > MAX_FEED_BYTES {
        return Err(AppError::InvalidArgument(format!(
            "feed {url}: body {} bytes exceeds cap {MAX_FEED_BYTES}",
            bytes.len()
        )));
    }
    let entries = parse_feed(&bytes)?;
    Ok(FeedPullOutcome {
        not_modified: false,
        bytes: bytes.len(),
        etag: etag_out,
        last_modified: lm_out,
        entries,
    })
}

/// Pure parse — no I/O.  Public for tests with canned XML.
pub fn parse_feed(bytes: &[u8]) -> AppResult<Vec<feed_rs::model::Entry>> {
    let parser = feed_rs::parser::Builder::new().build();
    let parsed = parser
        .parse(std::io::Cursor::new(bytes))
        .map_err(|e| AppError::Internal(format!("feed parse: {e}")))?;
    Ok(parsed.entries)
}

/// Convert one feed entry into (title, body) plain text.
pub fn entry_to_text(entry: &feed_rs::model::Entry) -> (String, String) {
    let title = entry
        .title
        .as_ref()
        .map(|t| t.content.clone())
        .unwrap_or_else(|| "untitled".into());
    let raw_body = entry
        .summary
        .as_ref()
        .map(|s| s.content.as_str())
        .or(entry
            .content
            .as_ref()
            .and_then(|c| c.body.as_deref()))
        .unwrap_or("");
    (title, strip_html(raw_body))
}

/// Tiny tag stripper — feed bodies often contain inline HTML.
pub fn strip_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if in_tag => {}
            _ => out.push(c),
        }
    }
    let mut compact = String::with_capacity(out.len());
    let mut prev_ws = false;
    for c in out.chars() {
        if c.is_whitespace() {
            if !prev_ws {
                compact.push(' ');
            }
            prev_ws = true;
        } else {
            compact.push(c);
            prev_ws = false;
        }
    }
    compact.trim().to_string()
}

/// Persist one feed entry.  Writes a temp `.txt` file under
/// `digest-cache/`, then routes through `knowledge_lite::ingest_path`
/// so the same dual-table FTS5 / RRF pipeline applies.  PII scrub
/// runs PRE-ingest.
pub async fn persist_feed_entry(
    feed_url: &str,
    source_id: &str,
    entry: &feed_rs::model::Entry,
) -> AppResult<u64> {
    let (title, body) = entry_to_text(entry);
    if body.trim().is_empty() {
        return Ok(0);
    }

    let (scrubbed, redactions) = scrub_for_ingest(&body);

    let cache = cache_dir()?;
    let slug = entry
        .id
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .take(80)
        .collect::<String>();
    let cache_path = cache.join(format!(
        "{}-{}.txt",
        sanitize_for_path(feed_url),
        if slug.is_empty() {
            "entry".to_string()
        } else {
            slug
        }
    ));
    let payload =
        format!("{title}\n\nSource: {feed_url}\n\n{scrubbed}\n");
    std::fs::write(&cache_path, payload).map_err(AppError::Io)?;

    // Ingest synchronously into the shared knowledge DB.  We swallow
    // limit errors so a single feed can't kill the daemon — the
    // knowledge stats UI surfaces them clearly.
    if let Err(e) = knowledge_lite::ingest_path_blocking(&cache_path) {
        tracing::warn!("digest ingest skip {}: {e}", cache_path.display());
    }

    append_history(&DigestEntry {
        source_id: source_id.to_string(),
        kind: "feed".into(),
        title: title.clone(),
        url_or_path: feed_url.to_string(),
        fetched_at: chrono::Utc::now(),
        pii_redactions: redactions,
        bytes: cache_path
            .metadata()
            .map(|m| m.len())
            .unwrap_or_default(),
    })?;
    Ok(redactions)
}
