<!-- SPDX-License-Identifier: MIT -->
<!--
  ChatMessage — single message bubble.

  - User messages anchor right with a neon-tinted border.
  - Assistant messages anchor left with a softer glass surface.
  - System messages render as muted center notices (rare in the UI today
    but still a valid `MessageRole` from the Rust side).
-->
<script lang="ts">
  import type { Message } from '$lib/stores/chat.svelte';

  interface Props {
    message: Message;
  }

  const { message }: Props = $props();

  const isUser = $derived(message.role === 'user');
  const isAssistant = $derived(message.role === 'assistant');
  const isSystem = $derived(message.role === 'system');
</script>

<div
  class="mb-3 flex w-full"
  class:justify-end={isUser}
  class:justify-start={isAssistant}
  class:justify-center={isSystem}
>
  {#if isSystem}
    <p class="font-mono text-xs text-impforge-text-secondary opacity-70">
      [system] {message.content}
    </p>
  {:else}
    <article
      class="max-w-[80%] rounded-xl border px-4 py-3 shadow-sm"
      class:bg-impforge-bg-secondary={isUser}
      class:border-impforge-neon={isUser}
      class:bg-impforge-bg-primary={isAssistant}
      class:border-impforge-border={isAssistant}
    >
      <p
        class="mb-1 font-mono text-[11px] uppercase tracking-wider"
        class:text-impforge-neon={isUser}
        class:text-impforge-magenta={isAssistant}
      >
        {message.role}
      </p>
      <p class="whitespace-pre-wrap break-words text-sm text-impforge-text-primary">
        {message.content}
      </p>
    </article>
  {/if}
</div>
