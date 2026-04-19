// SPDX-License-Identifier: MIT
// Chat store — Svelte 5 runes. Wires HyperChatPanel UI to the Rust
// backend (chat_lite + chat_session_memory) via Tauri invoke + Channel.
//
// Streaming model: the frontend creates a typed Channel<ChatChunk> and passes
// it to `chat_stream`; Rust pushes one chunk per Ollama NDJSON line; the
// Channel.onmessage handler accumulates into `partialResponse` and finalises
// when `done=true`.

import { invoke, Channel } from '@tauri-apps/api/core';
import { getProvidersState } from '$lib/stores/providers.svelte';

export type Role = 'user' | 'assistant' | 'system';

export interface Message {
  role: Role;
  content: string;
}

export interface ChatChunk {
  content: string;
  done: boolean;
}

export interface ThreadSnapshot {
  id: string;
  title: string;
  created_at: string;
  message_count: number;
}

interface ChatState {
  threadId: string | null;
  messages: Message[];
  streaming: boolean;
  partialResponse: string;
  lastError: string | null;
}

const state = $state<ChatState>({
  threadId: null,
  messages: [],
  streaming: false,
  partialResponse: '',
  lastError: null
});

/** Reactive snapshot of the chat state for use in components via `$derived`. */
export function getChatState(): ChatState {
  return state;
}

/**
 * Start a new thread. The Rust side returns a bare UUID string
 * (ThreadId is a serde-newtype-transparent String).
 */
export async function startNewThread(title: string = 'New chat'): Promise<void> {
  state.lastError = null;
  try {
    const id = await invoke<string>('session_create_thread', { title });
    state.threadId = id;
    state.messages = [];
    state.partialResponse = '';
  } catch (e) {
    state.lastError = String(e);
    throw e;
  }
}

/**
 * Send a user message, append it to the active thread, then stream the
 * assistant response token-by-token via a Tauri Channel.
 *
 * Lazily creates a thread on the first message of a session so the user can
 * land on /hyperchat and just start typing.
 */
export async function sendMessage(content: string): Promise<void> {
  if (!state.threadId) {
    await startNewThread();
  }
  const threadId = state.threadId;
  if (!threadId) {
    state.lastError = 'No active thread';
    return;
  }

  const userMsg: Message = { role: 'user', content };
  state.messages.push(userMsg);
  state.lastError = null;

  try {
    await invoke('session_append_message', {
      id: threadId,
      message: userMsg
    });
  } catch (e) {
    state.lastError = String(e);
    return;
  }

  state.streaming = true;
  state.partialResponse = '';

  const onChunk = new Channel<ChatChunk>();
  let finalisedThreadId = threadId;
  onChunk.onmessage = (chunk: ChatChunk) => {
    state.partialResponse += chunk.content;
    if (chunk.done) {
      const fullText = state.partialResponse;
      const assistantMsg: Message = { role: 'assistant', content: fullText };
      state.messages.push(assistantMsg);
      state.partialResponse = '';
      state.streaming = false;
      // Persist the final assistant message to the in-memory thread.
      // Best-effort — failures here only update lastError, never throw.
      void invoke('session_append_message', {
        id: finalisedThreadId,
        message: assistantMsg
      }).catch((e: unknown) => {
        state.lastError = String(e);
      });
    }
  };

  // Route to the selected BYOK provider when one is configured
  // (Feature 1 — multi-provider chat). Falls back to the legacy local-only
  // `chat_stream` (which always talks to Ollama) when no provider is picked
  // — same behavior the app shipped with at v0.2.0-alpha.
  try {
    const providers = getProvidersState();
    const useByok = providers.providers.length > 0;
    if (useByok) {
      await invoke('provider_chat_stream', {
        provider: providers.selectedProviderId,
        model: providers.selectedModel || null,
        messages: state.messages,
        onChunk
      });
    } else {
      await invoke('chat_stream', {
        messages: state.messages,
        model: null,
        onChunk
      });
    }
  } catch (e) {
    state.streaming = false;
    state.lastError = String(e);
  }
}

/** Clear the in-memory state without touching backend storage. */
export function clearLocalChat(): void {
  state.threadId = null;
  state.messages = [];
  state.partialResponse = '';
  state.streaming = false;
  state.lastError = null;
}

/** Fetch the list of thread snapshots from the backend. */
export async function listThreads(): Promise<ThreadSnapshot[]> {
  return await invoke<ThreadSnapshot[]>('session_list_threads');
}
