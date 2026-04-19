// SPDX-License-Identifier: MIT
// Slash-command catalog store — Svelte 5 runes. Loads the static catalog
// served by the Rust `slash_catalog` Tauri command and exposes a prefix
// filter for the ChatInput autocomplete.

import { invoke } from '@tauri-apps/api/core';

export interface SlashCommand {
  name: string;
  description: string;
  category: string;
}

export type SlashOutcome =
  | { kind: 'text'; text: string }
  | { kind: 'intent'; name: string; arg?: string }
  | { kind: 'unknown'; input: string };

const state = $state<{ commands: SlashCommand[]; loaded: boolean }>({
  commands: [],
  loaded: false
});

/** Reactive snapshot of the loaded catalog. */
export function getCommands(): SlashCommand[] {
  return state.commands;
}

/** Whether the catalog has been successfully loaded at least once. */
export function isLoaded(): boolean {
  return state.loaded;
}

/**
 * Fetch the catalog from the backend. Idempotent — repeated calls only
 * re-fetch when the previous load was empty.
 */
export async function loadCommands(): Promise<void> {
  if (state.loaded && state.commands.length > 0) {
    return;
  }
  const list = await invoke<SlashCommand[]>('slash_catalog');
  state.commands = list;
  state.loaded = true;
}

/**
 * Filter loaded commands by name prefix. Case-sensitive on purpose —
 * commands always use a leading `/` and lowercase letters.
 */
export function filterCommands(prefix: string): SlashCommand[] {
  const p = prefix.trim();
  if (!p.startsWith('/')) return [];
  return state.commands.filter((c) => c.name.startsWith(p));
}

/**
 * Dispatch a slash-command input via the Rust dispatcher. Returns the
 * tagged outcome so callers can branch on `kind`.
 */
export async function dispatch(
  input: string,
  threadId: string | null
): Promise<SlashOutcome> {
  return await invoke<SlashOutcome>('slash_dispatch', {
    input,
    threadId
  });
}
