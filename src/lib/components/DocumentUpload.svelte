<!-- SPDX-License-Identifier: MIT -->
<!--
  DocumentUpload — drag-drop zone for ingesting files into the local FTS5
  knowledge base.  Uses Tauri 2's onDragDropEvent with `getCurrentWebview`
  rather than a raw HTML drop handler (Tauri intercepts the drop before
  it reaches the DOM).

  Privacy reminder rendered in-component: every byte stays on the user's
  disk.  This text is part of the contract per REGEL 000-BRIDGE-NOT-PROCESS.
-->
<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import {
    getKnowledgeState,
    ingestPath,
    ingestDir,
    refreshStats,
  } from '$lib/stores/knowledge.svelte';

  const knowledge = $derived(getKnowledgeState());

  let isDragging = $state(false);
  let unlistenDrop: (() => void) | null = null;

  async function handlePaths(paths: string[]): Promise<void> {
    isDragging = false;
    for (const path of paths) {
      try {
        // Heuristic: treat path as directory if name has no dot OR ends with /.
        // Backend will error appropriately if wrong.
        if (path.endsWith('/') || path.endsWith('\\')) {
          await ingestDir(path, true);
        } else {
          await ingestPath(path);
        }
      } catch (e) {
        // Errors surface via the store — UI shows them in the error box.
        console.error('ingest failed:', e);
      }
    }
  }

  // Tauri's plugin-dialog isn't bundled in the MIT app build today —
  // the user can drop files directly OR paste an absolute path.  This
  // keeps the bundle small until the Pro app upgrades the dialog UX.
  let pathInput = $state('');

  async function ingestTypedPath(): Promise<void> {
    const trimmed = pathInput.trim();
    if (!trimmed) return;
    pathInput = '';
    if (trimmed.endsWith('/') || trimmed.endsWith('\\')) {
      await ingestDir(trimmed, true);
    } else {
      await ingestPath(trimmed);
    }
  }

  onMount(async () => {
    await refreshStats();
    try {
      const webview = getCurrentWebview();
      unlistenDrop = await webview.onDragDropEvent((evt) => {
        if (evt.payload.type === 'enter' || evt.payload.type === 'over') {
          isDragging = true;
        } else if (evt.payload.type === 'drop') {
          void handlePaths(evt.payload.paths);
        } else if (evt.payload.type === 'leave') {
          isDragging = false;
        }
      });
    } catch (e) {
      console.warn('drag-drop unavailable:', e);
    }
  });

  onDestroy(() => {
    if (unlistenDrop) unlistenDrop();
  });
</script>

<div
  class="rounded-2xl border-2 border-dashed p-8 transition-all duration-200 {isDragging
    ? 'border-impforge-neon bg-impforge-neon/5 shadow-[0_0_24px_rgba(0,255,102,0.25)]'
    : 'border-impforge-border-default'}"
>
  <div class="flex flex-col items-center gap-4">
    <div class="text-5xl text-impforge-text-secondary opacity-60">⤓</div>
    <div class="text-center">
      <h3 class="font-display text-2xl text-impforge-text-primary">
        Drop documents here
      </h3>
      <p class="mt-1 text-sm text-impforge-text-secondary">
        PDF · DOCX · XLSX · HTML · Markdown · plain text
      </p>
    </div>

    <div class="w-full flex gap-2">
      <input
        type="text"
        bind:value={pathInput}
        onkeydown={(e) => {
          if (e.key === 'Enter') void ingestTypedPath();
        }}
        placeholder="…or paste an absolute path (file or directory ending /)"
        class="flex-1 rounded-lg border border-impforge-border-default bg-impforge-bg-void px-3 py-2 font-mono text-xs text-impforge-text-primary placeholder-impforge-text-secondary focus:border-impforge-neon focus:outline-none"
      />
      <button
        type="button"
        onclick={ingestTypedPath}
        disabled={knowledge.ingesting || !pathInput.trim()}
        class="rounded-lg bg-impforge-neon px-4 py-2 font-mono text-sm text-impforge-bg-void transition hover:brightness-110 disabled:cursor-not-allowed disabled:opacity-50"
      >
        Ingest
      </button>
    </div>

    {#if knowledge.ingesting}
      <p class="font-mono text-xs text-impforge-text-secondary">Ingesting…</p>
    {/if}

    {#if knowledge.ingestError}
      <p class="rounded border border-impforge-magenta bg-impforge-magenta/10 px-3 py-2 text-xs text-impforge-magenta">
        {knowledge.ingestError}
      </p>
    {/if}

    {#if knowledge.lastOutcomes.length > 0}
      <ul class="mt-2 max-h-48 w-full overflow-y-auto font-mono text-xs">
        {#each knowledge.lastOutcomes.slice(0, 8) as o (o.path + o.doc_id)}
          <li class="flex items-center justify-between border-b border-impforge-border-default/40 py-1 text-impforge-text-secondary">
            <span class="truncate" title={o.path}>{o.path.split('/').pop()}</span>
            <span class="ml-2 shrink-0 text-impforge-cyan">
              {o.skipped_duplicate ? 'dup' : `${o.chunk_count} chunks`}
            </span>
          </li>
        {/each}
      </ul>
    {/if}

    <p class="mt-4 text-[10px] uppercase tracking-wider text-impforge-text-secondary opacity-60">
      Local-only · zero upload · per REGEL 000-BRIDGE-NOT-PROCESS
    </p>
  </div>
</div>
