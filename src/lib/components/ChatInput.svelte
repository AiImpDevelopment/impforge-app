<!-- SPDX-License-Identifier: MIT -->
<!--
  ChatInput — single-line input with slash-command autocomplete.

  - On `/` prefix, opens the autocomplete popover with prefix-filtered commands.
  - Enter submits; Shift+Enter inserts a newline (textarea grows up to 4 lines).
  - On submit, slash inputs are routed through the Rust `slash_dispatch` and
    surface their `text` outcome as a one-shot system message; intent and
    unknown outcomes are also surfaced as system messages so users see the
    backend's response without needing the dev console.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { sendMessage, getChatState } from '$lib/stores/chat.svelte';
  import {
    filterCommands,
    loadCommands,
    dispatch,
    type SlashCommand,
    type SlashOutcome
  } from '$lib/stores/slashCommands.svelte';

  let input = $state('');
  let busy = $state(false);

  const suggestions = $derived<SlashCommand[]>(
    input.startsWith('/') && input.length > 0 ? filterCommands(input) : []
  );
  const showSuggestions = $derived(suggestions.length > 0);

  onMount(() => {
    void loadCommands();
  });

  function pickSuggestion(cmd: SlashCommand): void {
    input = cmd.name + ' ';
  }

  function appendSystemNotice(text: string): void {
    const chat = getChatState();
    chat.messages.push({ role: 'system', content: text });
  }

  function describeOutcome(outcome: SlashOutcome): string {
    switch (outcome.kind) {
      case 'text':
        return outcome.text;
      case 'intent':
        return outcome.arg
          ? `Intent: ${outcome.name} (arg: ${outcome.arg})`
          : `Intent: ${outcome.name}`;
      case 'unknown':
        return `Unknown command: ${outcome.input}`;
    }
  }

  async function submit(): Promise<void> {
    const text = input.trim();
    if (!text || busy) return;
    busy = true;
    input = '';
    try {
      if (text.startsWith('/')) {
        const chat = getChatState();
        const outcome = await dispatch(text, chat.threadId);
        appendSystemNotice(describeOutcome(outcome));
      } else {
        await sendMessage(text);
      }
    } finally {
      busy = false;
    }
  }

  function onKeydown(e: KeyboardEvent): void {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      void submit();
    }
  }
</script>

<form
  onsubmit={(e) => {
    e.preventDefault();
    void submit();
  }}
  class="relative w-full"
>
  {#if showSuggestions}
    <div
      class="absolute bottom-full left-0 right-0 mb-2 max-h-48 overflow-auto rounded-md border border-impforge-border bg-impforge-bg-secondary shadow-xl"
    >
      {#each suggestions as cmd (cmd.name)}
        <button
          type="button"
          class="block w-full px-3 py-2 text-left transition-colors hover:bg-impforge-bg-primary focus:bg-impforge-bg-primary focus:outline-none"
          onclick={() => pickSuggestion(cmd)}
        >
          <span class="font-mono text-sm text-impforge-neon">{cmd.name}</span>
          <span class="ml-3 text-xs text-impforge-text-secondary">{cmd.description}</span>
          <span
            class="ml-2 rounded bg-impforge-bg-primary px-2 py-0.5 font-mono text-[10px] uppercase tracking-wider text-impforge-cyan"
          >
            {cmd.category}
          </span>
        </button>
      {/each}
    </div>
  {/if}

  <div class="flex items-end gap-2">
    <textarea
      bind:value={input}
      onkeydown={onKeydown}
      rows="1"
      placeholder="Type a message or /command (Enter to send, Shift+Enter for newline)"
      disabled={busy}
      class="flex-1 resize-none rounded-md border border-impforge-border bg-impforge-bg-secondary px-3 py-2 text-sm text-impforge-text-primary placeholder:text-impforge-text-secondary focus:border-impforge-neon focus:outline-none disabled:opacity-60"
    ></textarea>
    <button
      type="submit"
      disabled={busy || input.trim().length === 0}
      class="rounded-md border border-impforge-neon bg-impforge-neon px-4 py-2 text-sm font-semibold text-impforge-bg-primary transition-all hover:shadow-[0_0_16px_rgba(0,255,102,0.3)] focus:outline-none focus:ring-2 focus:ring-impforge-neon disabled:cursor-not-allowed disabled:opacity-50"
    >
      Send
    </button>
  </div>
</form>
