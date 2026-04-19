<!-- SPDX-License-Identifier: MIT -->
<!--
  HyperChatPanel — main MIT-side HyperChat surface.

  Layout:
    +----------------------------------------+
    | header (title + subtitle)   DigiImp 🟣 |
    +----------------------------------------+
    |                                        |
    |  scrollable message list               |
    |  + streaming partial bubble            |
    |                                        |
    +----------------------------------------+
    | ChatInput (autocomplete + send)        |
    +----------------------------------------+

  The store is consumed via $derived(getChatState()) so every
  state mutation in chat.svelte.ts re-renders this panel.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import ChatMessage from './ChatMessage.svelte';
  import ChatInput from './ChatInput.svelte';
  import DigiImpPlaceholder from './DigiImpPlaceholder.svelte';
  import { getChatState, clearLocalChat } from '$lib/stores/chat.svelte';

  const chat = $derived(getChatState());

  let scrollContainer: HTMLElement | null = $state(null);

  // Auto-scroll on new content (messages or streaming partial growth).
  $effect(() => {
    // Track the values that should trigger a scroll. Reading them inside the
    // effect is what registers the dependency.
    const _len = chat.messages.length;
    const _partial = chat.partialResponse;
    void _len;
    void _partial;
    void tick().then(() => {
      if (scrollContainer) {
        scrollContainer.scrollTop = scrollContainer.scrollHeight;
      }
    });
  });

  function onClear(): void {
    clearLocalChat();
  }
</script>

<div class="relative flex h-screen flex-col bg-impforge-bg-primary">
  <DigiImpPlaceholder />

  <header class="border-b border-impforge-border px-6 py-4">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="font-display text-2xl font-bold text-impforge-neon">HyperChat</h1>
        <p class="mt-1 text-sm text-impforge-text-secondary">
          Local Ollama chat — memory-only sessions
          <a
            href="https://impforge.com"
            class="ml-1 text-impforge-cyan underline-offset-2 hover:underline"
          >
            (Pro adds persistence)
          </a>
        </p>
      </div>
      <button
        type="button"
        onclick={onClear}
        class="rounded-md border border-impforge-border bg-impforge-bg-secondary px-3 py-1.5 font-mono text-xs uppercase tracking-wider text-impforge-text-secondary transition-colors hover:border-impforge-neon hover:text-impforge-neon focus:outline-none focus:ring-2 focus:ring-impforge-neon"
      >
        New chat
      </button>
    </div>
  </header>

  <main
    bind:this={scrollContainer}
    class="flex-1 overflow-y-auto px-6 py-4"
  >
    {#if chat.messages.length === 0 && !chat.streaming}
      <div class="mx-auto mt-16 max-w-md text-center">
        <p class="font-display text-lg text-impforge-text-primary">
          Start a conversation
        </p>
        <p class="mt-2 text-sm text-impforge-text-secondary">
          Type a message below or hit
          <code class="rounded bg-impforge-bg-secondary px-1.5 py-0.5 font-mono text-xs text-impforge-neon">/help</code>
          to see all available slash commands.
        </p>
      </div>
    {/if}

    {#each chat.messages as msg, i (i)}
      <ChatMessage message={msg} />
    {/each}

    {#if chat.streaming}
      <div class="mb-3 flex justify-start">
        <article
          class="max-w-[80%] rounded-xl border border-impforge-border bg-impforge-bg-primary px-4 py-3 shadow-sm"
        >
          <p class="mb-1 font-mono text-[11px] uppercase tracking-wider text-impforge-magenta">
            assistant <span class="opacity-60">(streaming…)</span>
          </p>
          <p class="whitespace-pre-wrap break-words text-sm text-impforge-text-primary">
            {chat.partialResponse}<span class="ml-0.5 inline-block h-3 w-2 animate-pulse bg-impforge-magenta"></span>
          </p>
        </article>
      </div>
    {/if}

    {#if chat.lastError}
      <div class="mb-3 rounded-md border border-red-700/50 bg-red-950/30 px-3 py-2">
        <p class="font-mono text-xs uppercase tracking-wider text-red-400">error</p>
        <p class="mt-1 text-xs text-red-200">{chat.lastError}</p>
      </div>
    {/if}
  </main>

  <footer class="border-t border-impforge-border px-6 py-4">
    <ChatInput />
  </footer>
</div>
