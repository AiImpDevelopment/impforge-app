<!-- SPDX-License-Identifier: MIT -->
<!--
  Notes route — entry point for the local knowledge base.

  Layout:
    1. Drag-drop upload zone (DocumentUpload component).
    2. Stats tile (documents, chunks, languages, MIT-tier remaining quota).
    3. Quick-search field that routes to /search with the typed query.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import DocumentUpload from '$lib/components/DocumentUpload.svelte';
  import {
    getKnowledgeState,
    refreshStats,
  } from '$lib/stores/knowledge.svelte';

  const knowledge = $derived(getKnowledgeState());
  const stats = $derived(knowledge.stats);

  let quickQuery = $state('');

  function bytesToMb(n: number): string {
    return (n / (1024 * 1024)).toFixed(2);
  }

  function startQuickSearch(): void {
    if (!quickQuery.trim()) return;
    void goto(`/search?q=${encodeURIComponent(quickQuery.trim())}`);
  }

  onMount(() => {
    void refreshStats();
  });
</script>

<svelte:head>
  <title>Notes — ImpForge</title>
</svelte:head>

<div class="mx-auto max-w-5xl space-y-6 p-6">
  <header>
    <h1 class="font-display text-3xl text-impforge-neon">Local knowledge base</h1>
    <p class="mt-1 text-sm text-impforge-text-secondary">
      Drop PDFs, Word docs, spreadsheets, HTML, Markdown or text files. Search them
      with FTS5 + Reciprocal Rank Fusion — every byte stays on your disk.
    </p>
  </header>

  <DocumentUpload />

  <section class="rounded-xl border border-impforge-border-default bg-impforge-bg-secondary p-5">
    <h2 class="mb-3 font-display text-xl text-impforge-text-primary">Index stats</h2>
    {#if stats}
      <dl class="grid grid-cols-2 gap-4 sm:grid-cols-4">
        <div>
          <dt class="font-mono text-[10px] uppercase text-impforge-text-secondary">
            Documents
          </dt>
          <dd class="font-display text-2xl text-impforge-cyan">
            {stats.document_count}
          </dd>
        </div>
        <div>
          <dt class="font-mono text-[10px] uppercase text-impforge-text-secondary">
            Chunks
          </dt>
          <dd class="font-display text-2xl text-impforge-cyan">
            {stats.chunk_count.toLocaleString()}
          </dd>
        </div>
        <div>
          <dt class="font-mono text-[10px] uppercase text-impforge-text-secondary">
            Indexed (MB)
          </dt>
          <dd class="font-display text-2xl text-impforge-cyan">
            {bytesToMb(stats.total_bytes)}
          </dd>
        </div>
        <div>
          <dt class="font-mono text-[10px] uppercase text-impforge-text-secondary">
            MIT quota left
          </dt>
          <dd class="font-display text-2xl text-impforge-magenta">
            {bytesToMb(stats.remaining_bytes)} MB
          </dd>
        </div>
      </dl>
      {#if stats.languages.length > 0}
        <div class="mt-4">
          <dt class="font-mono text-[10px] uppercase text-impforge-text-secondary">
            Languages
          </dt>
          <dd class="mt-1 flex flex-wrap gap-2">
            {#each stats.languages as [lang, count] (lang)}
              <span class="rounded bg-impforge-cyan/10 px-2 py-1 font-mono text-xs text-impforge-cyan">
                {lang} · {count}
              </span>
            {/each}
          </dd>
        </div>
      {/if}
    {:else}
      <p class="font-mono text-xs text-impforge-text-secondary">No documents indexed yet.</p>
    {/if}
  </section>

  <section class="rounded-xl border border-impforge-border-default bg-impforge-bg-secondary p-5">
    <h2 class="mb-3 font-display text-xl text-impforge-text-primary">Quick search</h2>
    <div class="flex gap-2">
      <input
        type="text"
        bind:value={quickQuery}
        onkeydown={(e) => {
          if (e.key === 'Enter') startQuickSearch();
        }}
        placeholder="What are you looking for?"
        class="flex-1 rounded-lg border border-impforge-border-default bg-impforge-bg-void px-3 py-2 font-mono text-sm text-impforge-text-primary placeholder-impforge-text-secondary focus:border-impforge-neon focus:outline-none"
      />
      <button
        type="button"
        onclick={startQuickSearch}
        disabled={!quickQuery.trim()}
        class="rounded-lg bg-impforge-neon px-4 py-2 font-mono text-sm text-impforge-bg-void transition hover:brightness-110 disabled:cursor-not-allowed disabled:opacity-50"
      >
        Search →
      </button>
    </div>
  </section>
</div>
