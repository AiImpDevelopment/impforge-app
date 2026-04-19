// SPDX-License-Identifier: MIT
//! Capability-diff viewer (rug-pull defense).
//!
//! Compares two `ServerCapabilities` snapshots — the version the user
//! consented to vs the version they're about to update to.  Computes
//! added/removed/changed tools + added/removed egress hosts +
//! added/removed filesystem paths, and flags whether the diff is a
//! "privilege escalation" (broadens what the server can do).
//!
//! UI shows the diff BEFORE `installer::update_to_latest` is called.
//! User must click "Approve" to proceed — anchored to ETDI per
//! [arXiv 2506.01333](https://arxiv.org/html/2506.01333v1).
//!
//! ## Hash drift detection
//!
//! Each `ServerCapabilities` carries a `capability_hash`.  If the hash
//! between two consecutive snapshots of the SAME version differs, the
//! server has silently rewritten its tool list — that's a tool-poisoning
//! red flag (per Invariant Labs notification).  `is_silent_rewrite`
//! makes that distinction explicit.

use crate::mcp::types::{
    CapabilityDiff, ServerCapabilities, ToolDiff, ToolSpec,
};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

/// Compute a stable hash over a tool list — feeds `capability_hash`.
pub fn compute_capability_hash(tools: &[ToolSpec]) -> String {
    let mut sorted: Vec<&ToolSpec> = tools.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    let mut hasher = Sha256::new();
    for tool in sorted {
        hasher.update(tool.name.as_bytes());
        hasher.update(b"|");
        hasher.update(tool.description.as_bytes());
        hasher.update(b"|");
        hasher.update(canonical_string(&tool.input_schema).as_bytes());
        hasher.update(b"|");
        for h in &tool.egress_hosts {
            hasher.update(h.as_bytes());
            hasher.update(b",");
        }
        hasher.update(b"|");
        for p in &tool.fs_paths {
            hasher.update(p.as_bytes());
            hasher.update(b",");
        }
        hasher.update(b"||");
    }
    let bytes = hasher.finalize();
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn canonical_string(value: &serde_json::Value) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

/// Detect silent rewrite — same version label, different capability hash.
pub fn is_silent_rewrite(prev: &ServerCapabilities, next: &ServerCapabilities) -> bool {
    prev.server_version == next.server_version && prev.capability_hash != next.capability_hash
}

/// Compute the diff between two capability snapshots.
pub fn compare(prev: &ServerCapabilities, next: &ServerCapabilities) -> CapabilityDiff {
    let prev_names: HashSet<&str> = prev.tools.iter().map(|t| t.name.as_str()).collect();
    let next_names: HashSet<&str> = next.tools.iter().map(|t| t.name.as_str()).collect();

    let added_tools: Vec<ToolSpec> = next
        .tools
        .iter()
        .filter(|t| !prev_names.contains(t.name.as_str()))
        .cloned()
        .collect();
    let removed_tools: Vec<ToolSpec> = prev
        .tools
        .iter()
        .filter(|t| !next_names.contains(t.name.as_str()))
        .cloned()
        .collect();

    let mut changed_tools = Vec::new();
    for next_tool in &next.tools {
        if !prev_names.contains(next_tool.name.as_str()) {
            continue;
        }
        let prev_tool = match prev.tools.iter().find(|t| t.name == next_tool.name) {
            Some(t) => t,
            None => continue,
        };
        let mut changes = Vec::new();
        if prev_tool.description != next_tool.description {
            changes.push("description changed".into());
        }
        if canonical_string(&prev_tool.input_schema) != canonical_string(&next_tool.input_schema) {
            changes.push("input schema changed".into());
        }
        let prev_egress: HashSet<&String> = prev_tool.egress_hosts.iter().collect();
        let next_egress: HashSet<&String> = next_tool.egress_hosts.iter().collect();
        if prev_egress != next_egress {
            let added: Vec<&&String> = next_egress.difference(&prev_egress).collect();
            let removed: Vec<&&String> = prev_egress.difference(&next_egress).collect();
            if !added.is_empty() {
                changes.push(format!(
                    "egress hosts added: {}",
                    added
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            if !removed.is_empty() {
                changes.push(format!(
                    "egress hosts removed: {}",
                    removed
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
        let prev_fs: HashSet<&String> = prev_tool.fs_paths.iter().collect();
        let next_fs: HashSet<&String> = next_tool.fs_paths.iter().collect();
        if prev_fs != next_fs {
            let added: Vec<&&String> = next_fs.difference(&prev_fs).collect();
            let removed: Vec<&&String> = prev_fs.difference(&next_fs).collect();
            if !added.is_empty() {
                changes.push(format!(
                    "fs paths added: {}",
                    added
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            if !removed.is_empty() {
                changes.push(format!(
                    "fs paths removed: {}",
                    removed
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
        if !changes.is_empty() {
            changed_tools.push(ToolDiff {
                name: next_tool.name.clone(),
                from: prev_tool.clone(),
                to: next_tool.clone(),
                changes,
            });
        }
    }

    // Aggregate added/removed egress + fs paths across the whole server.
    let prev_all_egress: HashSet<&String> = prev
        .tools
        .iter()
        .flat_map(|t| &t.egress_hosts)
        .collect();
    let next_all_egress: HashSet<&String> = next
        .tools
        .iter()
        .flat_map(|t| &t.egress_hosts)
        .collect();
    let added_egress_hosts: Vec<String> = next_all_egress
        .difference(&prev_all_egress)
        .map(|s| (*s).clone())
        .collect();
    let removed_egress_hosts: Vec<String> = prev_all_egress
        .difference(&next_all_egress)
        .map(|s| (*s).clone())
        .collect();

    let prev_all_fs: HashSet<&String> = prev.tools.iter().flat_map(|t| &t.fs_paths).collect();
    let next_all_fs: HashSet<&String> = next.tools.iter().flat_map(|t| &t.fs_paths).collect();
    let added_fs_paths: Vec<String> = next_all_fs
        .difference(&prev_all_fs)
        .map(|s| (*s).clone())
        .collect();
    let removed_fs_paths: Vec<String> = prev_all_fs
        .difference(&next_all_fs)
        .map(|s| (*s).clone())
        .collect();

    let privilege_escalation = !added_tools.is_empty()
        || !added_egress_hosts.is_empty()
        || !added_fs_paths.is_empty()
        || changed_tools.iter().any(|t| {
            t.changes.iter().any(|c| {
                c.contains("egress hosts added") || c.contains("fs paths added")
            })
        });

    CapabilityDiff {
        server_id: next.server_id.clone(),
        from_version: prev.server_version.clone(),
        to_version: next.server_version.clone(),
        added_tools,
        removed_tools,
        changed_tools,
        added_egress_hosts,
        removed_egress_hosts,
        added_fs_paths,
        removed_fs_paths,
        privilege_escalation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn snapshot(id: &str, version: &str, tools: Vec<ToolSpec>) -> ServerCapabilities {
        let hash = compute_capability_hash(&tools);
        ServerCapabilities {
            server_id: id.into(),
            server_version: version.into(),
            tools,
            resources: vec![],
            prompts: vec![],
            capability_hash: hash,
            fetched_at: Utc::now(),
        }
    }

    fn tool(name: &str, desc: &str) -> ToolSpec {
        ToolSpec {
            name: name.into(),
            description: desc.into(),
            input_schema: serde_json::json!({}),
            egress_hosts: vec![],
            fs_paths: vec![],
        }
    }

    #[test]
    fn capability_hash_invariant_under_tool_order() {
        let a = vec![tool("read", "x"), tool("write", "y")];
        let b = vec![tool("write", "y"), tool("read", "x")];
        assert_eq!(compute_capability_hash(&a), compute_capability_hash(&b));
    }

    #[test]
    fn diff_added_tool_marks_privilege_escalation() {
        let prev = snapshot("fs", "0.1", vec![tool("read", "x")]);
        let next = snapshot("fs", "0.2", vec![tool("read", "x"), tool("write", "y")]);
        let diff = compare(&prev, &next);
        assert_eq!(diff.added_tools.len(), 1);
        assert_eq!(diff.added_tools[0].name, "write");
        assert!(diff.privilege_escalation);
    }

    #[test]
    fn diff_removed_tool_does_not_escalate_privilege() {
        let prev = snapshot("fs", "0.1", vec![tool("read", "x"), tool("write", "y")]);
        let next = snapshot("fs", "0.2", vec![tool("read", "x")]);
        let diff = compare(&prev, &next);
        assert_eq!(diff.removed_tools.len(), 1);
        assert_eq!(diff.removed_tools[0].name, "write");
        assert!(!diff.privilege_escalation);
    }

    #[test]
    fn diff_egress_host_added_escalates_privilege() {
        let mut t = tool("send", "send-data");
        let prev = snapshot("net", "0.1", vec![t.clone()]);
        t.egress_hosts.push("evil.com".into());
        let next = snapshot("net", "0.2", vec![t]);
        let diff = compare(&prev, &next);
        assert!(!diff.added_egress_hosts.is_empty());
        assert!(diff.privilege_escalation);
    }

    #[test]
    fn diff_changed_description_recorded_but_no_escalation() {
        let prev = snapshot("fs", "0.1", vec![tool("read", "old desc")]);
        let next = snapshot("fs", "0.2", vec![tool("read", "new desc")]);
        let diff = compare(&prev, &next);
        assert_eq!(diff.changed_tools.len(), 1);
        assert!(diff
            .changed_tools[0]
            .changes
            .iter()
            .any(|c| c.contains("description changed")));
        assert!(!diff.privilege_escalation);
    }

    #[test]
    fn silent_rewrite_detected_when_same_version_different_hash() {
        let prev = snapshot("fs", "1.0", vec![tool("read", "a")]);
        let next = snapshot("fs", "1.0", vec![tool("read", "MODIFIED")]);
        assert!(is_silent_rewrite(&prev, &next));
    }

    #[test]
    fn silent_rewrite_false_for_different_versions() {
        let prev = snapshot("fs", "1.0", vec![tool("read", "a")]);
        let next = snapshot("fs", "1.1", vec![tool("read", "MODIFIED")]);
        assert!(!is_silent_rewrite(&prev, &next));
    }

    #[test]
    fn diff_no_changes_returns_empty_lists() {
        let snap = snapshot("fs", "0.1", vec![tool("read", "x")]);
        let diff = compare(&snap, &snap);
        assert!(diff.added_tools.is_empty());
        assert!(diff.removed_tools.is_empty());
        assert!(diff.changed_tools.is_empty());
        assert!(!diff.privilege_escalation);
    }
}
