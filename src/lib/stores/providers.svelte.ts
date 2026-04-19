// SPDX-License-Identifier: MIT
// Provider list reactive store — Svelte 5 runes.
// Wires the Settings UI + ProviderPicker to the Rust backend
// (`providers.rs`) via Tauri `invoke`. The store NEVER holds a key value —
// only a `KeyStatus` enum.

import { invoke } from '@tauri-apps/api/core';

export type ProviderId = 'ollama' | 'openai' | 'anthropic' | 'gemini' | 'openrouter';
export type KeyStatus = 'set' | 'missing' | 'local';

export interface ProviderInfo {
  id: ProviderId;
  display_name: string;
  default_model: string;
  requires_key: boolean;
  key_status: KeyStatus;
}

interface ProvidersState {
  providers: ProviderInfo[];
  loading: boolean;
  lastError: string | null;
  selectedProviderId: ProviderId;
  selectedModel: string;
}

const state = $state<ProvidersState>({
  providers: [],
  loading: false,
  lastError: null,
  selectedProviderId: 'ollama',
  selectedModel: 'qwen3:8b'
});

/** Reactive snapshot of the providers state for use in components via `$derived`. */
export function getProvidersState(): ProvidersState {
  return state;
}

/** Fetch the latest provider list from the backend (key statuses only). */
export async function refreshProviders(): Promise<void> {
  state.loading = true;
  state.lastError = null;
  try {
    state.providers = await invoke<ProviderInfo[]>('provider_list');
  } catch (e) {
    state.lastError = String(e);
  } finally {
    state.loading = false;
  }
}

/** Save (or replace) a key for `providerId`. Refresh after success. */
export async function addProviderKey(providerId: ProviderId, key: string): Promise<void> {
  state.lastError = null;
  try {
    await invoke('provider_add', { provider: providerId, key });
    await refreshProviders();
  } catch (e) {
    state.lastError = String(e);
    throw e;
  }
}

/** Remove the stored key for `providerId`. Idempotent. */
export async function removeProviderKey(providerId: ProviderId): Promise<void> {
  state.lastError = null;
  try {
    await invoke('provider_remove', { provider: providerId });
    await refreshProviders();
  } catch (e) {
    state.lastError = String(e);
    throw e;
  }
}

/**
 * Pick a provider for outgoing chats. Updates the default model to that
 * provider's recommended pick. Components that care should `$derived` from
 * `state.selectedProviderId` / `state.selectedModel`.
 */
export function selectProvider(providerId: ProviderId): void {
  state.selectedProviderId = providerId;
  const info = state.providers.find((p) => p.id === providerId);
  if (info) {
    state.selectedModel = info.default_model;
  }
}

/** Override the model independently of the provider selection. */
export function selectModel(model: string): void {
  state.selectedModel = model;
}
