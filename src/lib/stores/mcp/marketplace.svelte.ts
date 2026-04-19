// SPDX-License-Identifier: MIT
// Marketplace store — drives BrowseView + DetailView.

import { invoke } from '@tauri-apps/api/core';

export type Category = 'files' | 'web' | 'comms' | 'code' | 'data' | 'ai' | 'custom';

export type Transport = 'stdio' | 'streamable_http' | 'sse';

export interface CapabilityHint {
  tools: number;
  resources: number;
  prompts: number;
}

export interface EnvVarSpec {
  name: string;
  description: string;
  secret: boolean;
  required: boolean;
}

export interface MarketplaceEntry {
  id: string;
  name: string;
  publisher: string;
  version: string;
  description: string;
  category: Category;
  tags: string[];
  transport: Transport;
  command: string | null;
  args: string[];
  env_schema: EnvVarSpec[];
  homepage: string | null;
  license: string | null;
  install_count: number;
  stars: number;
  verified: boolean;
  signed: boolean;
  oauth: boolean;
  last_updated: string;
  capability_hint: CapabilityHint;
}

export interface TrustScore {
  score: number;
  verified_pts: number;
  signed_pts: number;
  installs_pts: number;
  stars_pts: number;
  recency_pts: number;
  sandbox_pts: number;
}

export interface OfflineStatus {
  entries_cached: number;
  last_refresh: string | null;
  db_path: string;
  has_network_baseline: boolean;
}

export interface BrowseFilters {
  search?: string;
  category?: string;
  limit?: number;
  verified_only?: boolean;
  signed_only?: boolean;
  no_oauth?: boolean;
}

interface MarketplaceState {
  entries: MarketplaceEntry[];
  selectedId: string | null;
  query: string;
  category: Category | null;
  verifiedOnly: boolean;
  signedOnly: boolean;
  noOauth: boolean;
  loading: boolean;
  offline: OfflineStatus | null;
  lastError: string | null;
  trustScores: Record<string, TrustScore>;
}

export const marketplace = $state<MarketplaceState>({
  entries: [],
  selectedId: null,
  query: '',
  category: null,
  verifiedOnly: false,
  signedOnly: false,
  noOauth: false,
  loading: false,
  offline: null,
  lastError: null,
  trustScores: {},
});

export async function refresh(): Promise<void> {
  marketplace.loading = true;
  marketplace.lastError = null;
  try {
    const filters: BrowseFilters = {
      search: marketplace.query || undefined,
      category: marketplace.category ?? undefined,
      verified_only: marketplace.verifiedOnly,
      signed_only: marketplace.signedOnly,
      no_oauth: marketplace.noOauth,
    };
    marketplace.entries = await invoke<MarketplaceEntry[]>('mcp_marketplace_browse', { filters });
  } catch (err) {
    marketplace.lastError = String(err);
    marketplace.entries = [];
  } finally {
    marketplace.loading = false;
  }
}

export async function syncMirror(): Promise<number> {
  marketplace.loading = true;
  marketplace.lastError = null;
  try {
    const count = await invoke<number>('mcp_marketplace_sync_mirror', { url: null });
    await refreshOfflineStatus();
    await refresh();
    return count;
  } catch (err) {
    marketplace.lastError = String(err);
    return 0;
  } finally {
    marketplace.loading = false;
  }
}

export async function refreshOfflineStatus(): Promise<void> {
  try {
    marketplace.offline = await invoke<OfflineStatus>('mcp_marketplace_get_offline_status');
  } catch (err) {
    marketplace.lastError = String(err);
    marketplace.offline = null;
  }
}

export async function getEntry(id: string): Promise<MarketplaceEntry | null> {
  try {
    return await invoke<MarketplaceEntry>('mcp_marketplace_get', { id });
  } catch (err) {
    marketplace.lastError = String(err);
    return null;
  }
}

export async function getTrustScore(id: string): Promise<TrustScore | null> {
  if (marketplace.trustScores[id]) {
    return marketplace.trustScores[id];
  }
  try {
    const score = await invoke<TrustScore>('mcp_trust_score', { id });
    marketplace.trustScores[id] = score;
    return score;
  } catch (err) {
    marketplace.lastError = String(err);
    return null;
  }
}

export function setQuery(q: string): void {
  marketplace.query = q;
}

export function setCategory(c: Category | null): void {
  marketplace.category = c;
}

export function toggleVerified(): void {
  marketplace.verifiedOnly = !marketplace.verifiedOnly;
}

export function toggleSigned(): void {
  marketplace.signedOnly = !marketplace.signedOnly;
}

export function toggleNoOauth(): void {
  marketplace.noOauth = !marketplace.noOauth;
}

export function selectEntry(id: string | null): void {
  marketplace.selectedId = id;
}
