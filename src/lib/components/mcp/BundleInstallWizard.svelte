<!-- SPDX-License-Identifier: MIT -->
<!--
  3-click bundle wizard.  Card 1: pick a bundle.  Card 2: review.
  Card 3: progress + completion.  The Aha moment for first-time users.
-->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  interface BundleTemplate {
    id: string;
    name: string;
    description: string;
    icon: string;
    server_ids: string[];
    prefilled_env: [string, string, string][];
  }

  interface BundleInstallResult {
    server_id: string;
    outcome: { Ok?: { installed: { id: string }; warnings: string[] } } | { Err?: string };
  }

  interface Props {
    onClose?: () => void;
    onComplete?: () => void;
  }

  let { onClose, onComplete }: Props = $props();

  let bundles = $state<BundleTemplate[]>([]);
  let selectedId = $state<string | null>(null);
  let installing = $state(false);
  let results = $state<BundleInstallResult[]>([]);
  let loadError = $state<string | null>(null);

  const selected = $derived(bundles.find((b) => b.id === selectedId) ?? null);
  const totalServers = $derived(selected?.server_ids.length ?? 0);
  const completedCount = $derived(results.length);
  const successCount = $derived(results.filter((r) => 'Ok' in r.outcome).length);

  $effect(() => {
    void loadBundles();
  });

  async function loadBundles(): Promise<void> {
    try {
      bundles = await invoke<BundleTemplate[]>('mcp_bundle_list');
    } catch (err) {
      loadError = String(err);
    }
  }

  async function install(): Promise<void> {
    if (!selectedId) return;
    installing = true;
    results = [];
    try {
      results = await invoke<BundleInstallResult[]>('mcp_bundle_install', {
        bundleId: selectedId,
      });
    } catch (err) {
      loadError = String(err);
    } finally {
      installing = false;
    }
  }

  function reset(): void {
    selectedId = null;
    results = [];
    loadError = null;
    installing = false;
  }
</script>

<section class="wizard">
  <header>
    <h2>Quick-start bundles</h2>
    <p class="hint">Three clicks to a working agent.  Each bundle installs 2-3 curated MCP servers.</p>
  </header>

  {#if loadError}
    <div class="error mono">{loadError}</div>
  {/if}

  {#if results.length === 0}
    <div class="grid">
      {#each bundles as b (b.id)}
        <button
          type="button"
          class="bundle-card"
          class:selected={selectedId === b.id}
          onclick={() => (selectedId = b.id)}
          aria-pressed={selectedId === b.id}
        >
          <div class="icon" data-icon={b.icon}>
            <svg viewBox="0 0 24 24" width="24" height="24" aria-hidden="true">
              <circle cx="12" cy="12" r="10" fill="none" stroke="currentColor" stroke-width="1.5" />
              <path
                d="M8 12l3 3 5-5"
                fill="none"
                stroke="currentColor"
                stroke-width="1.5"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
            </svg>
          </div>
          <h3>{b.name}</h3>
          <p>{b.description}</p>
          <p class="server-list mono">{b.server_ids.join(' · ')}</p>
        </button>
      {/each}
    </div>

    <footer>
      <button type="button" class="btn ghost" onclick={() => onClose?.()}>Cancel</button>
      <button
        type="button"
        class="btn primary"
        onclick={install}
        disabled={!selectedId || installing}
      >
        {installing ? 'Installing…' : 'Install bundle'}
      </button>
    </footer>
  {:else}
    <section class="results">
      <h3>Installation complete</h3>
      <p class="result-summary">
        {successCount}/{totalServers} server{totalServers === 1 ? '' : 's'} installed
      </p>
      <ul>
        {#each results as r (r.server_id)}
          <li class:success={'Ok' in r.outcome} class:failure={!('Ok' in r.outcome)}>
            <span class="status mono">
              {'Ok' in r.outcome ? '✓' : '✗'}
            </span>
            <span class="id mono">{r.server_id}</span>
            {#if !('Ok' in r.outcome) && 'Err' in r.outcome}
              <span class="err">{(r.outcome as { Err: string }).Err}</span>
            {/if}
          </li>
        {/each}
      </ul>
      <footer>
        <button type="button" class="btn ghost" onclick={reset}>Install another</button>
        <button
          type="button"
          class="btn primary"
          onclick={() => {
            onComplete?.();
            onClose?.();
          }}
        >
          Done
        </button>
      </footer>
    </section>
  {/if}

  {#if installing}
    <div class="progress">
      <div class="bar"></div>
      <p class="hint mono">{completedCount}/{totalServers} servers installed</p>
    </div>
  {/if}
</section>

<style>
  .wizard {
    background: linear-gradient(135deg, rgba(19, 19, 26, 0.95) 0%, rgba(13, 13, 18, 0.98) 100%);
    border: 1px solid #2A2A3A;
    border-radius: 14px;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 18px;
    backdrop-filter: blur(16px) saturate(120%);
  }
  header h2 {
    font-family: 'Space Grotesk', system-ui, sans-serif;
    font-weight: 600;
    margin: 0 0 4px 0;
    color: #E8E8ED;
    font-size: 22px;
  }
  .hint {
    color: #A0A0B0;
    font-family: 'Inter', sans-serif;
    font-size: 12px;
    margin: 0;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 12px;
  }
  .bundle-card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    align-items: flex-start;
    padding: 16px;
    background: rgba(19, 19, 26, 0.6);
    border: 1px solid #2A2A3A;
    border-radius: 10px;
    cursor: pointer;
    text-align: left;
    transition: all 0.2s;
    color: inherit;
  }
  .bundle-card:hover {
    border-color: rgba(0, 255, 102, 0.4);
    transform: translateY(-1px);
    box-shadow: 0 0 16px rgba(0, 255, 102, 0.1);
  }
  .bundle-card.selected {
    border-color: #00FF66;
    background: rgba(0, 255, 102, 0.06);
    box-shadow: 0 0 16px rgba(0, 255, 102, 0.2);
  }
  .icon {
    color: #00CCFF;
    margin-bottom: 4px;
  }
  .bundle-card.selected .icon {
    color: #00FF66;
  }
  .bundle-card h3 {
    margin: 0;
    font-family: 'Space Grotesk', sans-serif;
    color: #E8E8ED;
    font-size: 14px;
    font-weight: 600;
  }
  .bundle-card p {
    margin: 0;
    color: #A0A0B0;
    font-family: 'Inter', sans-serif;
    font-size: 12px;
    line-height: 1.5;
  }
  .server-list {
    color: #606070;
    font-size: 11px;
    margin-top: 4px;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    padding-top: 12px;
    border-top: 1px solid #1F1F2A;
  }
  .btn {
    padding: 9px 18px;
    border-radius: 8px;
    font-family: 'Space Grotesk', sans-serif;
    font-weight: 600;
    font-size: 13px;
    cursor: pointer;
    border: 1px solid transparent;
    transition: all 0.2s;
  }
  .btn.ghost {
    background: transparent;
    color: #A0A0B0;
    border-color: #2A2A3A;
  }
  .btn.ghost:hover {
    color: #E8E8ED;
    border-color: #A0A0B0;
  }
  .btn.primary {
    background: linear-gradient(135deg, #00FF66 0%, #00CC52 100%);
    color: #0D0D12;
  }
  .btn.primary:hover:not(:disabled) {
    box-shadow: 0 0 16px rgba(0, 255, 102, 0.4);
  }
  .btn.primary:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  .results {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .results h3 {
    font-family: 'Space Grotesk', sans-serif;
    color: #E8E8ED;
    margin: 0;
    font-size: 16px;
    font-weight: 600;
  }
  .result-summary {
    color: #00FF66;
    font-family: 'JetBrains Mono', monospace;
    font-size: 14px;
    margin: 0;
  }
  ul {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 12px;
    border-radius: 6px;
    background: rgba(19, 19, 26, 0.6);
    border: 1px solid #1F1F2A;
  }
  li.success .status {
    color: #00FF66;
  }
  li.failure {
    border-color: rgba(255, 51, 102, 0.4);
  }
  li.failure .status {
    color: #FF3366;
  }
  .id {
    color: #E8E8ED;
    font-size: 13px;
  }
  .err {
    color: #FF3366;
    font-size: 11px;
    font-family: 'JetBrains Mono', monospace;
    margin-left: auto;
  }
  .progress {
    display: flex;
    flex-direction: column;
    gap: 6px;
    align-items: center;
  }
  .bar {
    width: 80%;
    height: 3px;
    background: linear-gradient(90deg, transparent, #00FF66, transparent);
    background-size: 200% 100%;
    animation: scan 1.4s linear infinite;
    border-radius: 999px;
  }
  .error {
    color: #FF3366;
    background: rgba(255, 51, 102, 0.08);
    border: 1px solid rgba(255, 51, 102, 0.4);
    padding: 10px;
    border-radius: 8px;
    font-size: 12px;
  }
  .mono {
    font-family: 'JetBrains Mono', ui-monospace, monospace;
  }
  @keyframes scan {
    from { background-position: -100% 0; }
    to { background-position: 200% 0; }
  }
  @media (prefers-reduced-motion: reduce) {
    .bar { animation: none; }
    .bundle-card,
    .btn { transition: none; }
  }
</style>
