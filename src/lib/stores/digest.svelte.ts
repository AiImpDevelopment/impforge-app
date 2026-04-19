// SPDX-License-Identifier: MIT
// Digest store — Svelte 5 runes. Wires DigestSourceManager,
// DigestActivity, DigestPrivacyGate to the Rust backend
// (auto_digest_lite + digest_clipboard + digest_screenshots +
// digest_browser) via Tauri invoke.

import { invoke } from '@tauri-apps/api/core';

// ─── Type mirrors of the Rust types ─────────────────────────────────────

export type DigestSource =
  | { kind: 'feed'; id: string; url: string; interval_secs: number; last_modified?: string | null; etag?: string | null; last_polled_unix?: number }
  | { kind: 'folder'; id: string; path: string; recursive: boolean; allow_ext?: string[] }
  | { kind: 'clipboard'; id: string }
  | { kind: 'screenshots'; id: string; path?: string | null }
  | { kind: 'browser'; id: string; family: string };

export interface DigestEntry {
  source_id: string;
  kind: string;
  title: string;
  url_or_path: string;
  fetched_at: string;
  pii_redactions: number;
  bytes: number;
}

export interface DigestStats {
  source_count: number;
  feed_count: number;
  folder_count: number;
  clipboard_active: boolean;
  screenshots_active: boolean;
  paused: boolean;
  quiet_hours_enabled: boolean;
  history_rows: number;
  total_pii_redactions: number;
}

export interface RunOnceSummary {
  feeds_pulled: number;
  feeds_unchanged: number;
  entries_persisted: number;
  folders_swept: number;
  files_indexed: number;
  chunks_indexed: number;
  pii_redactions: number;
  clipboard_active: boolean;
  screenshots_active: boolean;
  errors: number;
}

interface DigestState {
  sources: DigestSource[];
  history: DigestEntry[];
  stats: DigestStats | null;
  loading: boolean;
  error: string | null;
  // Live status for the opt-in extras
  clipboardActive: boolean;
  screenshotsActive: boolean;
  // Last run-once summary for the activity panel
  lastSummary: RunOnceSummary | null;
}

const state = $state<DigestState>({
  sources: [],
  history: [],
  stats: null,
  loading: false,
  error: null,
  clipboardActive: false,
  screenshotsActive: false,
  lastSummary: null,
});

export function getDigestState(): DigestState {
  return state;
}

// ─── Actions ────────────────────────────────────────────────────────────

export async function refreshAll(): Promise<void> {
  state.loading = true;
  state.error = null;
  try {
    const [sources, stats, history, clipboard, screenshots] = await Promise.all([
      invoke<DigestSource[]>('digest_list_sources'),
      invoke<DigestStats>('digest_stats'),
      invoke<DigestEntry[]>('digest_history', { limit: 50 }),
      invoke<boolean>('digest_clipboard_status').catch(() => false),
      invoke<boolean>('digest_screenshots_status').catch(() => false),
    ]);
    state.sources = sources;
    state.stats = stats;
    state.history = history;
    state.clipboardActive = clipboard;
    state.screenshotsActive = screenshots;
  } catch (err) {
    state.error = String(err);
  } finally {
    state.loading = false;
  }
}

export async function addFeed(url: string, intervalSecs = 300): Promise<void> {
  await invoke('digest_add_source', {
    source: {
      kind: 'feed',
      id: 'placeholder',
      url,
      interval_secs: intervalSecs,
    },
  });
  await refreshAll();
}

export async function addFolder(
  path: string,
  recursive: boolean,
  allowExt: string[],
): Promise<void> {
  await invoke('digest_add_source', {
    source: {
      kind: 'folder',
      id: 'placeholder',
      path,
      recursive,
      allow_ext: allowExt,
    },
  });
  await refreshAll();
}

export async function removeSource(id: string): Promise<void> {
  await invoke('digest_remove_source', { id });
  await refreshAll();
}

export async function runOnce(): Promise<RunOnceSummary> {
  const summary = await invoke<RunOnceSummary>('digest_run_once');
  state.lastSummary = summary;
  await refreshAll();
  return summary;
}

export async function pause(): Promise<void> {
  await invoke('digest_pause');
  await refreshAll();
}

export async function resume(): Promise<void> {
  await invoke('digest_resume');
  await refreshAll();
}

export async function setQuietHours(
  enabled: boolean,
  start: number,
  end: number,
): Promise<void> {
  await invoke('digest_set_quiet_hours', { enabled, start, end });
  await refreshAll();
}

export async function enableClipboard(): Promise<void> {
  await invoke('digest_clipboard_enable');
  await refreshAll();
}

export async function disableClipboard(): Promise<void> {
  await invoke('digest_clipboard_disable');
  await refreshAll();
}

export async function enableScreenshots(folder?: string): Promise<void> {
  await invoke('digest_screenshots_enable', { folder });
  await refreshAll();
}

export async function disableScreenshots(): Promise<void> {
  await invoke('digest_screenshots_disable');
  await refreshAll();
}

export async function defaultScreenshotsFolder(): Promise<string | null> {
  return await invoke<string | null>('digest_screenshots_default_folder');
}

export async function importBrowserBookmarks(family: string): Promise<number> {
  const redactions = await invoke<number>('digest_browser_import_bookmarks', { family });
  await refreshAll();
  return redactions;
}

export async function importBrowserHistory(
  family: string,
  days: number,
): Promise<number> {
  const redactions = await invoke<number>('digest_browser_import_history', { family, days });
  await refreshAll();
  return redactions;
}
