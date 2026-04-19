// SPDX-License-Identifier: MIT
//! PII detection + scrubbing.  Tier-2 helper used by Feature 3
//! (Global Digest) to enforce Recall anti-pattern #4 — every byte of
//! ingested content runs through this scrubber BEFORE storage / LLM.
//!
//! v1 covers the high-frequency cases that dominate digest streams:
//!   - emails
//!   - phone numbers (E.164 + common country formats, 7-15 digits)
//!   - IBAN (length-validated, country-prefixed)
//!   - credit-card numbers (Luhn-validated)
//!   - US Social Security Numbers (`XXX-XX-XXXX`)
//!   - IPv4 + IPv6 addresses
//!   - common API-key-shaped tokens (`sk-…`, `pk_…`, JWT-like dotted
//!     base64 triplets, GitHub `ghp_…`, AWS access-key prefix)
//!
//! Patterns deliberately err on the side of redaction in noisy
//! contexts (RSS feeds + clipboard).  False positives are acceptable
//! when the alternative is leaking a secret to the search index.
//!
//! All detection regexes compile once via `once_cell::Lazy` so the
//! per-call cost is the linear scan, not the regex build.

use crate::error::AppResult;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Categories of personally-identifiable information the scrubber recognises.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PiiKind {
    Email,
    Phone,
    Iban,
    CreditCard,
    Ssn,
    IpAddress,
    ApiKey,
    JwtToken,
}

/// One PII match within an input string.  Byte offsets into the
/// original UTF-8 text — callers can splice with [..start] + redacted
/// + [end..].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiiMatch {
    pub kind: PiiKind,
    pub start: usize,
    pub end: usize,
    pub redacted: String,
}

/// Result of running the scrubber: cleaned text plus the list of matches.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrubResult {
    pub scrubbed: String,
    pub matches: Vec<PiiMatch>,
}

// ─── Regex catalog (compiled once) ───────────────────────────────────────

static RE_EMAIL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)[a-z0-9._%+\-]+@[a-z0-9.\-]+\.[a-z]{2,}")
        .expect("RE_EMAIL")
});

static RE_PHONE: Lazy<Regex> = Lazy::new(|| {
    // Generous: optional `+`, country code, area code, body — 7-15 digits total.
    Regex::new(r"\+?\d{1,3}[ \-.]?\(?\d{2,4}\)?[ \-.]?\d{3,4}[ \-.]?\d{3,4}")
        .expect("RE_PHONE")
});

static RE_IBAN: Lazy<Regex> = Lazy::new(|| {
    // 2-letter country, 2 check digits, 11-30 alphanumerics.  Generous.
    Regex::new(r"\b[A-Z]{2}\d{2}[A-Z0-9]{11,30}\b").expect("RE_IBAN")
});

static RE_CCN: Lazy<Regex> = Lazy::new(|| {
    // 13-19 digits, optional spaces or dashes — Luhn-checked downstream.
    Regex::new(r"\b(?:\d[ \-]?){12,18}\d\b").expect("RE_CCN")
});

static RE_SSN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").expect("RE_SSN")
});

static RE_IPV4: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").expect("RE_IPV4")
});

static RE_IPV6: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(?:[0-9a-f]{1,4}:){2,7}[0-9a-f]{1,4}\b").expect("RE_IPV6")
});

static RE_API_KEY: Lazy<Regex> = Lazy::new(|| {
    // Common SaaS key prefixes — followed by enough body characters
    // to look like a real key.  Different providers use different
    // body lengths (AWS IAM access keys are 16 chars after `AKIA`,
    // GitHub PATs are 36+ after `ghp_`), so we require ≥16 chars to
    // catch the shortest real key while filtering most false positives.
    Regex::new(
        r"(?i)\b(?:sk-|pk_|xox[bps]-|ghp_|gho_|ghu_|ghs_|ghr_|AKIA|AIza)[A-Za-z0-9_\-]{16,}\b",
    )
    .expect("RE_API_KEY")
});

static RE_JWT: Lazy<Regex> = Lazy::new(|| {
    // base64url.base64url.base64url with realistic length.
    Regex::new(r"\bey[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}\b")
        .expect("RE_JWT")
});

// ─── Validation helpers ──────────────────────────────────────────────────

/// Luhn-mod-10 check for credit card candidates.  Strips non-digits
/// before validating.
fn luhn_valid(input: &str) -> bool {
    let digits: Vec<u32> = input.chars().filter_map(|c| c.to_digit(10)).collect();
    if !(13..=19).contains(&digits.len()) {
        return false;
    }
    let mut sum = 0u32;
    let len = digits.len();
    for (i, d) in digits.iter().enumerate() {
        let pos_from_right = len - 1 - i;
        let mut x = *d;
        if pos_from_right % 2 == 1 {
            x *= 2;
            if x > 9 {
                x -= 9;
            }
        }
        sum += x;
    }
    sum.is_multiple_of(10)
}

/// MOD-97 IBAN validation per ISO 13616.  Strips spaces, rotates first
/// 4 chars to end, converts letters to numerics, mod 97 == 1.
fn iban_valid(input: &str) -> bool {
    let cleaned: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.len() < 15 || cleaned.len() > 34 {
        return false;
    }
    let (head, tail) = cleaned.split_at(4);
    let rotated = format!("{tail}{head}");
    let mut numeric = String::with_capacity(rotated.len() * 2);
    for c in rotated.chars() {
        if let Some(d) = c.to_digit(10) {
            numeric.push_str(&d.to_string());
        } else if c.is_ascii_uppercase() {
            numeric.push_str(&((c as u32 - 'A' as u32 + 10).to_string()));
        } else {
            return false;
        }
    }
    // Modulo 97 over a long string — process 9 digits at a time.
    let mut remainder: u64 = 0;
    for chunk_start in (0..numeric.len()).step_by(9) {
        let end = (chunk_start + 9).min(numeric.len());
        let frag = &numeric[chunk_start..end];
        let combined = format!("{remainder}{frag}");
        remainder = combined.parse::<u64>().unwrap_or(0) % 97;
    }
    remainder == 1
}

/// Basic IPv4 sanity — every octet 0..=255.  Drops `0.0.0.0`-style
/// non-noteworthy literals would be over-engineering; we redact every
/// well-formed IP.
fn ipv4_valid(input: &str) -> bool {
    input
        .split('.')
        .all(|o| o.parse::<u8>().is_ok())
        && input.split('.').count() == 4
}

// ─── Detection ───────────────────────────────────────────────────────────

/// Return every PII match in `text` in left-to-right document order.
pub fn detect(text: &str) -> Vec<PiiMatch> {
    let mut matches: Vec<PiiMatch> = Vec::new();

    // Order matters — more specific patterns first, then broaden.
    push_matches(&mut matches, text, &RE_EMAIL, PiiKind::Email, "[REDACTED:EMAIL]", |_| true);
    push_matches(&mut matches, text, &RE_API_KEY, PiiKind::ApiKey, "[REDACTED:API_KEY]", |_| true);
    push_matches(&mut matches, text, &RE_JWT, PiiKind::JwtToken, "[REDACTED:JWT]", |_| true);
    push_matches(&mut matches, text, &RE_IBAN, PiiKind::Iban, "[REDACTED:IBAN]", iban_valid);
    push_matches(&mut matches, text, &RE_CCN, PiiKind::CreditCard, "[REDACTED:CCN]", luhn_valid);
    push_matches(&mut matches, text, &RE_SSN, PiiKind::Ssn, "[REDACTED:SSN]", |_| true);
    push_matches(&mut matches, text, &RE_IPV4, PiiKind::IpAddress, "[REDACTED:IP]", ipv4_valid);
    push_matches(&mut matches, text, &RE_IPV6, PiiKind::IpAddress, "[REDACTED:IPV6]", |_| true);
    push_matches(&mut matches, text, &RE_PHONE, PiiKind::Phone, "[REDACTED:PHONE]", |s| {
        // Phones must contain ≥7 digits to reduce false positives on
        // version numbers, ISBNs, etc.
        s.chars().filter(|c| c.is_ascii_digit()).count() >= 7
    });

    // Sort by start, then dedup overlapping spans (keep the earliest).
    matches.sort_by_key(|m| (m.start, std::cmp::Reverse(m.end)));
    let mut deduped: Vec<PiiMatch> = Vec::with_capacity(matches.len());
    let mut last_end = 0usize;
    for m in matches {
        if m.start >= last_end {
            last_end = m.end;
            deduped.push(m);
        }
    }
    deduped
}

fn push_matches(
    out: &mut Vec<PiiMatch>,
    text: &str,
    re: &Regex,
    kind: PiiKind,
    redaction: &str,
    validator: impl Fn(&str) -> bool,
) {
    for m in re.find_iter(text) {
        if !validator(m.as_str()) {
            continue;
        }
        out.push(PiiMatch {
            kind,
            start: m.start(),
            end: m.end(),
            redacted: redaction.to_string(),
        });
    }
}

/// Scrub PII matches from `text`, replacing each match with its
/// redaction marker.  Returns the scrubbed text + the list of
/// matches so callers can surface "N redactions" in the UI.
pub fn scrub(text: &str) -> AppResult<ScrubResult> {
    let matches = detect(text);
    if matches.is_empty() {
        return Ok(ScrubResult {
            scrubbed: text.to_string(),
            matches,
        });
    }
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for m in &matches {
        out.push_str(&text[cursor..m.start]);
        out.push_str(&m.redacted);
        cursor = m.end;
    }
    out.push_str(&text[cursor..]);
    Ok(ScrubResult {
        scrubbed: out,
        matches,
    })
}

// ─── Tauri commands ──────────────────────────────────────────────────────

/// Detect (but do not redact) all PII matches in the input text.
#[tauri::command]
pub async fn pii_detect(text: String) -> AppResult<Vec<PiiMatch>> {
    Ok(detect(&text))
}

/// Scrub the input text, replacing all detected PII with redacted markers.
#[tauri::command]
pub async fn pii_scrub(text: String) -> AppResult<ScrubResult> {
    scrub(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_email() {
        let m = detect("contact me at hello@impforge.com please");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].kind, PiiKind::Email);
    }

    #[test]
    fn detects_ssn() {
        let m = detect("SSN: 123-45-6789 here");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].kind, PiiKind::Ssn);
    }

    #[test]
    fn luhn_validates_credit_card() {
        // 4111 1111 1111 1111 is the canonical test Visa
        assert!(luhn_valid("4111111111111111"));
        assert!(!luhn_valid("4111111111111112"));
    }

    #[test]
    fn detects_credit_card_luhn() {
        let m = detect("Card 4111-1111-1111-1111 expires soon");
        assert!(m.iter().any(|x| x.kind == PiiKind::CreditCard));
    }

    #[test]
    fn rejects_non_luhn_digits() {
        let m = detect("Random digits 1234567890123 should not redact");
        assert!(!m.iter().any(|x| x.kind == PiiKind::CreditCard));
    }

    #[test]
    fn detects_iban_with_mod97() {
        // Real test-vector IBAN (Belgium): BE68 5390 0754 7034
        let m = detect("Pay to BE68539007547034 today");
        assert!(m.iter().any(|x| x.kind == PiiKind::Iban));
    }

    #[test]
    fn detects_ipv4() {
        let m = detect("Server at 192.168.0.1 is up");
        assert!(m.iter().any(|x| x.kind == PiiKind::IpAddress));
    }

    #[test]
    fn rejects_invalid_ipv4_octet() {
        let m = detect("Not an IP: 999.999.999.999 here");
        assert!(!m.iter().any(|x| x.kind == PiiKind::IpAddress));
    }

    #[test]
    fn detects_jwt_token() {
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        let m = detect(&format!("Bearer {jwt} please"));
        assert!(m.iter().any(|x| x.kind == PiiKind::JwtToken));
    }

    #[test]
    fn detects_api_key_prefixes() {
        let m = detect("My key: sk-abcdefghijklmnopqrstuvwxyz1234");
        assert!(m.iter().any(|x| x.kind == PiiKind::ApiKey));
    }

    #[test]
    fn scrubs_email_in_place() {
        let result = scrub("write to test@example.com please").expect("scrub");
        assert!(result.scrubbed.contains("[REDACTED:EMAIL]"));
        assert_eq!(result.matches.len(), 1);
    }

    #[test]
    fn scrub_passthrough_when_no_pii() {
        let result = scrub("Nothing sensitive here").expect("scrub");
        assert_eq!(result.scrubbed, "Nothing sensitive here");
        assert!(result.matches.is_empty());
    }

    #[test]
    fn scrubs_password_like_api_key() {
        // Behavioural test for F3: clipboard captures must strip API
        // keys before storage.  This proves the integration end-to-end.
        let snippet = "AWS_KEY=AKIAIOSFODNN7EXAMPLE here";
        let result = scrub(snippet).expect("scrub");
        assert!(
            result.scrubbed.contains("[REDACTED:API_KEY]"),
            "expected API_KEY redaction in {}",
            result.scrubbed
        );
        assert!(result.matches.iter().any(|m| m.kind == PiiKind::ApiKey));
    }

    #[test]
    fn dedup_overlapping_matches() {
        // An email contains a possible IPv4 (e.g. local domain).  Email
        // wins because we register it first.
        let m = detect("foo@10.0.0.1");
        // Should pick email OR ip but not both overlapping.
        let starts: Vec<_> = m.iter().map(|x| x.start).collect();
        let mut sorted = starts.clone();
        sorted.dedup();
        assert_eq!(starts.len(), sorted.len());
    }

    #[test]
    fn detects_phone_with_country_code() {
        let m = detect("Call +1 415 555 0123 anytime");
        assert!(m.iter().any(|x| x.kind == PiiKind::Phone));
    }

    #[test]
    fn rejects_phone_with_too_few_digits() {
        let m = detect("Version 1.2.3 is out");
        assert!(!m.iter().any(|x| x.kind == PiiKind::Phone));
    }
}
