// SPDX-License-Identifier: MIT
// Per-provider spend reactive mirror — Svelte 5 runes.
// Surfaces the running telemetry from `spend.rs` to Settings + chat composer.
// All numbers are display-only — we never phone home.

import { invoke } from '@tauri-apps/api/core';

export interface ModelSpend {
  call_count: number;
  total_bytes: number;
  total_latency_ms: number;
}

export interface ProviderSpend {
  provider: string;
  call_count: number;
  total_bytes: number;
  total_latency_ms: number;
  by_model: Record<string, ModelSpend>;
}

interface SpendState {
  spend: ProviderSpend[];
  loading: boolean;
  lastError: string | null;
}

const state = $state<SpendState>({
  spend: [],
  loading: false,
  lastError: null
});

export function getSpendState(): SpendState {
  return state;
}

/** Refresh from the backend. Cheap — just a memory read on the Rust side. */
export async function refreshSpend(): Promise<void> {
  state.loading = true;
  state.lastError = null;
  try {
    state.spend = await invoke<ProviderSpend[]>('spend_get_all');
  } catch (e) {
    state.lastError = String(e);
  } finally {
    state.loading = false;
  }
}

/** Reset all running totals. Returns success boolean for caller UX. */
export async function resetSpend(): Promise<boolean> {
  state.lastError = null;
  try {
    await invoke('spend_reset');
    await refreshSpend();
    return true;
  } catch (e) {
    state.lastError = String(e);
    return false;
  }
}
