// SPDX-License-Identifier: MIT
// Knowledge store — Svelte 5 runes. Wires DocumentUpload, Search and Notes
// pages to the Rust backend (knowledge_lite + document_parse) via Tauri
// invoke. State is shared so a successful ingest in DocumentUpload
// re-renders the Search/Notes lists without a manual refetch.

import { invoke } from '@tauri-apps/api/core';

export interface KnowledgeEntry {
  id: string;
  source: string;
  title: string;
  body: string;
  ingested_at: string;
}

export interface SearchResult {
  entry: KnowledgeEntry;
  rank: number;
  snippet: string;
  sub_scores: {
    porter_rank: number | null;
    trigram_rank: number | null;
    format: string;
  };
  line_start: number;
  line_end: number;
}

export interface IngestOutcome {
  doc_id: number;
  path: string;
  format: string;
  language: string;
  chunk_count: number;
  bytes: number;
  skipped_duplicate: boolean;
}

export interface KnowledgeStats {
  document_count: number;
  chunk_count: number;
  total_bytes: number;
  languages: Array<[string, number]>;
  remaining_documents_estimate: number;
  remaining_bytes: number;
}

export interface CitationPreview {
  doc_path: string;
  format: string;
  line_start: number;
  line_end: number;
  text: string;
  page_image_png_b64: string | null;
}

interface KnowledgeState {
  // Search
  query: string;
  results: SearchResult[];
  searching: boolean;
  searchError: string | null;
  // Ingest
  ingesting: boolean;
  lastOutcomes: IngestOutcome[];
  ingestError: string | null;
  // Citation
  citation: CitationPreview | null;
  citationLoading: boolean;
  // Stats
  stats: KnowledgeStats | null;
  // Pro teaser
  proTeaserCount: number;
}

const state = $state<KnowledgeState>({
  query: '',
  results: [],
  searching: false,
  searchError: null,
  ingesting: false,
  lastOutcomes: [],
  ingestError: null,
  citation: null,
  citationLoading: false,
  stats: null,
  proTeaserCount: 0,
});

export function getKnowledgeState(): KnowledgeState {
  return state;
}

export async function ingestPath(path: string): Promise<IngestOutcome> {
  state.ingesting = true;
  state.ingestError = null;
  try {
    const outcome = await invoke<IngestOutcome>('knowledge_ingest_path', { path });
    state.lastOutcomes = [outcome, ...state.lastOutcomes].slice(0, 50);
    await refreshStats();
    return outcome;
  } catch (e) {
    state.ingestError = String(e);
    throw e;
  } finally {
    state.ingesting = false;
  }
}

export async function ingestDir(path: string, recursive: boolean): Promise<IngestOutcome[]> {
  state.ingesting = true;
  state.ingestError = null;
  try {
    const outcomes = await invoke<IngestOutcome[]>('knowledge_ingest_dir', {
      path,
      recursive,
    });
    state.lastOutcomes = [...outcomes, ...state.lastOutcomes].slice(0, 50);
    await refreshStats();
    return outcomes;
  } catch (e) {
    state.ingestError = String(e);
    throw e;
  } finally {
    state.ingesting = false;
  }
}

export async function runSearch(query: string, limit = 10): Promise<void> {
  state.query = query;
  state.searching = true;
  state.searchError = null;
  try {
    const results = await invoke<SearchResult[]>('knowledge_search', {
      query,
      limit,
    });
    state.results = results;
    // Pro teaser — Pro overrides this command to return real counts.
    state.proTeaserCount = await invoke<number>('knowledge_pro_teaser_count', {
      query,
    });
  } catch (e) {
    state.searchError = String(e);
    state.results = [];
  } finally {
    state.searching = false;
  }
}

export async function refreshStats(): Promise<void> {
  try {
    state.stats = await invoke<KnowledgeStats>('knowledge_stats');
  } catch {
    state.stats = null;
  }
}

export async function loadCitation(docId: number, chunkIdx: number): Promise<void> {
  state.citationLoading = true;
  state.citation = null;
  try {
    state.citation = await invoke<CitationPreview>('knowledge_get_citation', {
      docId,
      chunkIdx,
    });
  } catch (e) {
    state.citation = null;
    console.error('citation load failed:', e);
  } finally {
    state.citationLoading = false;
  }
}

export async function deleteDoc(docId: number): Promise<void> {
  await invoke('knowledge_delete_doc', { docId });
  await refreshStats();
  // Refresh search results to drop deleted hits.
  if (state.query) {
    await runSearch(state.query);
  }
}

export function clearCitation(): void {
  state.citation = null;
}
