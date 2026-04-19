<!-- SPDX-License-Identifier: MIT -->
<!--
  Search route — hybrid FTS5 (porter ⊕ trigram) via Reciprocal Rank Fusion.

  - Reads `?q=` from the URL on mount.
  - Renders ranked results with snippet + per-retriever sub-score badges
    (the "why this result?" tooltip).
  - Click a result → loadCitation populates CitationPreview side-panel.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/state';
  import CitationPreview from '$lib/components/CitationPreview.svelte';
  import {
    getKnowledgeState,
    runSearch,
    loadCitation,
    type SearchResult,
  } from '$lib/stores/knowledge.svelte';

  const knowledge = $derived(getKnowledgeState());

  let inputValue = $state('');

  function executeSearch(): void {
    if (!inputValue.trim()) return;
    void runSearch(inputValue.trim(), 20);
  }

  function pickResult(r: SearchResult): void {
    void loadCitation(Number(r.entry.id), 0);
  }

  function badgeColor(rank: number | null): string {
    if (rank === null) return 'bg-impforge-text-secondary/20 text-impforge-text-secondary';
    if (rank <= 3) return 'bg-impforge-neon/20 text-impforge-neon';
    if (rank <= 10) return 'bg-impforge-cyan/20 text-impforge-cyan';
    return 'bg-impforge-magenta/20 text-impforge-magenta';
  }

  onMount(() => {
    const q = page.url.searchParams.get('q');
    if (q) {
      inputValue = q;
      void runSearch(q, 20);
    }
  });
</script>

<svelte:head>
  <title>Search — ImpForge</title>
</svelte:head>

<div class="mx-auto grid max-w-7xl gap-6 p-6 lg:grid-cols-[3fr_2fr]">
  <div class="space-y-6">
    <header>
      <h1 class="font-display text-3xl text-impforge-neon">Hybrid search</h1>
      <p class="mt-1 text-sm text-impforge-text-secondary">
        FTS5 porter ⊕ trigram via Reciprocal Rank Fusion — bilingual-friendly,
        Umlaut-safe, citation-ready.
      </p>
    </header>

    <div class="flex gap-2">
      <input
        type="text"
        bind:value={inputValue}
        onkeydown={(e) => {
          if (e.key === 'Enter') executeSearch();
        }}
        placeholder='try "rust", "Künstlich" or "deploy AND container"'
        class="flex-1 rounded-lg border border-impforge-border-default bg-impforge-bg-void px-3 py-2 font-mono text-sm text-impforge-text-primary placeholder-impforge-text-secondary focus:border-impforge-neon focus:outline-none"
      />
      <button
        type="button"
        onclick={executeSearch}
        disabled={knowledge.searching || !inputValue.trim()}
        class="rounded-lg bg-impforge-neon px-4 py-2 font-mono text-sm text-impforge-bg-void transition hover:brightness-110 disabled:cursor-not-allowed disabled:opacity-50"
      >
        {knowledge.searching ? 'Searching…' : 'Search'}
      </button>
    </div>

    {#if knowledge.searchError}
      <p class="rounded border border-impforge-magenta bg-impforge-magenta/10 px-3 py-2 font-mono text-xs text-impforge-magenta">
        {knowledge.searchError}
      </p>
    {/if}

    {#if knowledge.proTeaserCount > 0}
      <p class="rounded border border-impforge-purple bg-impforge-purple/10 px-3 py-2 font-mono text-xs text-impforge-purple">
        +{knowledge.proTeaserCount} additional matches via vector + Knowledge Graph in
        <a href="https://impforge.com" class="underline">ImpForge Pro</a>.
      </p>
    {/if}

    {#if knowledge.results.length === 0 && !knowledge.searching && knowledge.query}
      <p class="font-mono text-sm text-impforge-text-secondary">
        No matches for "{knowledge.query}".
      </p>
    {/if}

    <ul class="space-y-4">
      {#each knowledge.results as r, i (r.entry.id + r.line_start)}
        <li>
          <button
            type="button"
            onclick={() => pickResult(r)}
            class="block w-full text-left rounded-lg border border-impforge-border-default bg-impforge-bg-secondary p-4 transition hover:border-impforge-neon hover:shadow-[0_0_16px_rgba(0,255,102,0.2)]"
          >
            <div class="mb-2 flex items-center gap-2">
              <span class="font-mono text-[10px] text-impforge-text-secondary">#{i + 1}</span>
              <span class="font-display text-lg text-impforge-text-primary">
                {r.entry.title || r.entry.source.split('/').pop()}
              </span>
              <span class="ml-auto font-mono text-[10px] text-impforge-text-secondary">
                lines {r.line_start}–{r.line_end}
              </span>
            </div>
            <p class="font-mono text-sm text-impforge-text-primary opacity-90">
              {r.snippet}
            </p>
            <div class="mt-3 flex flex-wrap items-center gap-2">
              <span class="rounded px-2 py-0.5 font-mono text-[10px] {badgeColor(r.sub_scores.porter_rank)}">
                porter#{r.sub_scores.porter_rank ?? '–'}
              </span>
              <span class="rounded px-2 py-0.5 font-mono text-[10px] {badgeColor(r.sub_scores.trigram_rank)}">
                trigram#{r.sub_scores.trigram_rank ?? '–'}
              </span>
              <span class="rounded bg-impforge-text-secondary/10 px-2 py-0.5 font-mono text-[10px] text-impforge-text-secondary">
                {r.sub_scores.format}
              </span>
              <span class="ml-auto font-mono text-[10px] text-impforge-text-secondary">
                rrf {r.rank.toFixed(4)}
              </span>
            </div>
          </button>
        </li>
      {/each}
    </ul>
  </div>

  <div class="lg:sticky lg:top-6 lg:self-start">
    <CitationPreview />
  </div>
</div>
