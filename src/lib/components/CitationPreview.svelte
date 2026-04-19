<!-- SPDX-License-Identifier: MIT -->
<!--
  CitationPreview — shows the source chunk that backed a search result.

  For PDFs, the backend renders the page bitmap via pdfium-render; we
  display it side-by-side with the chunk text and the line range.
  For non-PDFs, we render text-only with the file path and line range.
-->
<script lang="ts">
  import {
    getKnowledgeState,
    clearCitation,
  } from '$lib/stores/knowledge.svelte';

  const knowledge = $derived(getKnowledgeState());
  const citation = $derived(knowledge.citation);
</script>

{#if knowledge.citationLoading}
  <div class="rounded-lg border border-impforge-cyan/40 bg-impforge-bg-secondary p-4">
    <p class="font-mono text-xs text-impforge-cyan">Loading citation…</p>
  </div>
{:else if citation}
  <div class="rounded-lg border border-impforge-border-default bg-impforge-bg-secondary p-4">
    <div class="mb-3 flex items-center justify-between">
      <h4 class="font-display text-lg text-impforge-text-primary">Citation</h4>
      <button
        type="button"
        onclick={clearCitation}
        class="font-mono text-xs text-impforge-text-secondary hover:text-impforge-magenta"
        aria-label="Close citation"
      >
        ✕
      </button>
    </div>

    <div class="grid grid-cols-1 gap-4 lg:grid-cols-{citation.page_image_png_b64 ? '2' : '1'}">
      <div>
        <p class="mb-2 font-mono text-[10px] uppercase tracking-wider text-impforge-text-secondary">
          {citation.format} · lines {citation.line_start}–{citation.line_end}
        </p>
        <pre class="max-h-72 overflow-y-auto whitespace-pre-wrap rounded bg-impforge-bg-void p-3 font-mono text-xs text-impforge-text-primary">{citation.text}</pre>
        <p class="mt-2 truncate font-mono text-[10px] text-impforge-text-secondary" title={citation.doc_path}>
          {citation.doc_path}
        </p>
      </div>

      {#if citation.page_image_png_b64}
        <div>
          <p class="mb-2 font-mono text-[10px] uppercase tracking-wider text-impforge-text-secondary">
            Page preview (pdfium)
          </p>
          <img
            src="data:image/png;base64,{citation.page_image_png_b64}"
            alt="PDF page preview"
            class="max-h-96 w-full rounded border border-impforge-border-default object-contain"
          />
        </div>
      {/if}
    </div>
  </div>
{/if}
