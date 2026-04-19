<!-- SPDX-License-Identifier: MIT -->
<script lang="ts">
  import { getDigestState, refreshAll, runOnce } from '$lib/stores/digest.svelte';

  const digest = $derived(getDigestState());

  let busy = $state(false);
  let error = $state<string | null>(null);

  const recent = $derived(digest.history.slice(0, 50));
  const totalRedactions = $derived(
    recent.reduce((acc, r) => acc + r.pii_redactions, 0),
  );

  async function handleRunOnce() {
    busy = true;
    error = null;
    try {
      await runOnce();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  function fmtBytes(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
    return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  }

  function fmtDate(s: string): string {
    try {
      const d = new Date(s);
      return d.toLocaleString();
    } catch {
      return s;
    }
  }

  $effect(() => {
    refreshAll();
  });
</script>

<section class="digest-activity">
  <header>
    <h2>Recent activity</h2>
    <div class="badges">
      <span class="badge cyan" title="Locally encrypted at rest (AES-GCM)">
        Locally encrypted (AES-GCM)
      </span>
      <span class="badge magenta" title="Total PII redactions in this view">
        {totalRedactions} PII redactions
      </span>
      <span class="badge green" title="Zero telemetry">
        Zero telemetry
      </span>
    </div>
    <button onclick={handleRunOnce} disabled={busy}>
      {busy ? 'Running…' : 'Run all sources once'}
    </button>
  </header>

  {#if error}
    <div class="error">{error}</div>
  {/if}

  {#if digest.lastSummary}
    <div class="summary">
      <div><strong>Feeds:</strong> {digest.lastSummary.feeds_pulled} pulled · {digest.lastSummary.feeds_unchanged} unchanged</div>
      <div><strong>Folders:</strong> {digest.lastSummary.folders_swept} swept · {digest.lastSummary.files_indexed} files · {digest.lastSummary.chunks_indexed} chunks</div>
      <div><strong>PII redactions:</strong> {digest.lastSummary.pii_redactions}</div>
      <div><strong>Errors:</strong> {digest.lastSummary.errors}</div>
    </div>
  {/if}

  <div class="activity-list">
    {#if recent.length === 0}
      <p class="empty">
        No activity yet. Add a source above + click "Run all sources once".
      </p>
    {:else}
      {#each recent as row, i (row.fetched_at + i)}
        <article class="activity-row" class:has-pii={row.pii_redactions > 0}>
          <header class="row-header">
            <span class="kind">{row.kind}</span>
            <span class="title">{row.title}</span>
            <span class="meta">{fmtDate(row.fetched_at)}</span>
          </header>
          <div class="row-body">
            <div class="provenance" title="Provenance: source URL or path">
              {row.url_or_path}
            </div>
            <div class="counters">
              {#if row.pii_redactions > 0}
                <span class="pii-badge" title="PII redacted before storage">
                  {row.pii_redactions} PII redacted
                </span>
              {/if}
              <span class="bytes">{fmtBytes(row.bytes)}</span>
            </div>
          </div>
        </article>
      {/each}
    {/if}
  </div>
</section>

<style>
  .digest-activity {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    padding: 1.5rem;
    color: #e8e8ed;
  }
  header {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.75rem;
  }
  h2 {
    margin: 0;
    flex: 1;
    font-family: 'Space Grotesk', sans-serif;
  }
  .badges {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }
  .badge {
    font-size: 0.7rem;
    padding: 0.25rem 0.6rem;
    border-radius: 999px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .badge.cyan {
    background: rgba(0, 204, 255, 0.15);
    color: #00ccff;
    border: 1px solid rgba(0, 204, 255, 0.3);
  }
  .badge.magenta {
    background: rgba(255, 51, 153, 0.15);
    color: #ff3399;
    border: 1px solid rgba(255, 51, 153, 0.3);
  }
  .badge.green {
    background: rgba(0, 255, 102, 0.15);
    color: #00ff66;
    border: 1px solid rgba(0, 255, 102, 0.3);
  }
  button {
    padding: 0.5rem 1rem;
    background: #00ff66;
    color: #08080d;
    border: 1px solid #00ff66;
    border-radius: 4px;
    cursor: pointer;
    font-weight: 600;
  }
  button:hover:not(:disabled) {
    box-shadow: 0 0 16px rgba(0, 255, 102, 0.4);
  }
  button:disabled {
    background: #2a2a3a;
    color: #a0a0b0;
    cursor: not-allowed;
  }
  .summary {
    background: #13131a;
    border: 1px solid #2a2a3a;
    border-radius: 6px;
    padding: 0.75rem 1rem;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 0.5rem 1rem;
    font-family: 'JetBrains Mono', monospace;
    font-size: 0.85rem;
  }
  .summary strong {
    color: #00ccff;
    font-weight: 600;
  }
  .activity-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .activity-row {
    background: #13131a;
    border: 1px solid #2a2a3a;
    border-radius: 6px;
    padding: 0.75rem 1rem;
    transition: border-color 0.2s;
  }
  .activity-row.has-pii {
    border-left: 3px solid #ff3399;
  }
  .row-header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex-wrap: wrap;
  }
  .kind {
    font-family: 'JetBrains Mono', monospace;
    font-size: 0.75rem;
    padding: 0.15rem 0.5rem;
    background: #08080d;
    color: #00ccff;
    border-radius: 4px;
    text-transform: uppercase;
  }
  .title {
    flex: 1;
    font-weight: 600;
  }
  .meta {
    font-size: 0.75rem;
    color: #a0a0b0;
  }
  .row-body {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    margin-top: 0.5rem;
    font-size: 0.8rem;
  }
  .provenance {
    color: #a0a0b0;
    font-family: 'JetBrains Mono', monospace;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
  }
  .counters {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }
  .pii-badge {
    background: rgba(255, 51, 153, 0.15);
    color: #ff3399;
    padding: 0.15rem 0.5rem;
    border-radius: 4px;
    font-weight: 600;
  }
  .bytes {
    color: #a0a0b0;
    font-family: 'JetBrains Mono', monospace;
  }
  .empty {
    color: #a0a0b0;
    font-style: italic;
    text-align: center;
    padding: 2rem;
  }
  .error {
    background: rgba(255, 51, 51, 0.1);
    border: 1px solid rgba(255, 51, 51, 0.3);
    border-radius: 6px;
    color: #ff8888;
    padding: 0.75rem 1rem;
  }
</style>
