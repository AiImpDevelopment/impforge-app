<!-- SPDX-License-Identifier: MIT -->
<!--
  Tabbed shell for the MCP settings panel.  Three tabs:
    Browse · Installed · Health
  Each tab is its own +page.svelte route — the parent only owns the
  navigation bar + offline-status indicator.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import {
    marketplace,
    refreshOfflineStatus,
    syncMirror,
  } from '$lib/stores/mcp/marketplace.svelte';

  type Tab = 'browse' | 'installed' | 'health';

  let activeTab = $state<Tab>('browse');
  let syncing = $state(false);

  onMount(() => {
    void refreshOfflineStatus();
    activeTab = 'browse';
    // Auto-route to the BrowseView so this page acts as the canonical
    // entry point — navigating to /settings/mcp lands you on Browse.
    goto('/settings/mcp/browse');
  });

  function navigate(tab: Tab): void {
    activeTab = tab;
    if (tab === 'browse') goto('/settings/mcp/browse');
    else if (tab === 'installed') goto('/settings/mcp/installed');
    else if (tab === 'health') goto('/settings/mcp/health');
  }

  async function refresh(): Promise<void> {
    syncing = true;
    try {
      await syncMirror();
    } finally {
      syncing = false;
    }
  }

  const lastRefreshLabel = $derived.by(() => {
    if (!marketplace.offline?.last_refresh) return 'never synced — using bundled seed';
    const dt = new Date(marketplace.offline.last_refresh);
    if (Number.isNaN(dt.getTime())) return 'recently';
    const days = Math.floor((Date.now() - dt.getTime()) / (1000 * 60 * 60 * 24));
    if (days < 1) return 'today';
    if (days === 1) return 'yesterday';
    return `${days}d ago`;
  });
</script>

<div class="shell">
  <header class="hero">
    <div>
      <h1>MCP plugins</h1>
      <p class="subtitle">
        Browse 6 000+ servers · install with PKCE OAuth · tamper-evident
        invocation log · works fully offline.
      </p>
    </div>
    <div class="offline">
      <div class="offline-card" class:fresh={marketplace.offline && marketplace.offline.entries_cached > 0}>
        <span class="dot" aria-hidden="true"></span>
        <div class="offline-meta">
          <strong class="mono">{marketplace.offline?.entries_cached ?? 0}</strong>
          <span>entries cached · synced {lastRefreshLabel}</span>
        </div>
        <button type="button" class="sync" onclick={refresh} disabled={syncing}>
          {syncing ? 'syncing…' : 'sync'}
        </button>
      </div>
    </div>
  </header>

  <nav class="tabs">
    <button
      type="button"
      class="tab"
      class:active={activeTab === 'browse'}
      onclick={() => navigate('browse')}
      aria-current={activeTab === 'browse' ? 'page' : undefined}
    >
      Browse
    </button>
    <button
      type="button"
      class="tab"
      class:active={activeTab === 'installed'}
      onclick={() => navigate('installed')}
      aria-current={activeTab === 'installed' ? 'page' : undefined}
    >
      Installed
    </button>
    <button
      type="button"
      class="tab"
      class:active={activeTab === 'health'}
      onclick={() => navigate('health')}
      aria-current={activeTab === 'health' ? 'page' : undefined}
    >
      Health
    </button>
  </nav>

  <p class="hint">Use the tabs above to browse, manage, or monitor your MCP servers.</p>
</div>

<style>
  .shell {
    background:
      radial-gradient(ellipse at top, rgba(0, 255, 102, 0.04), transparent 60%),
      linear-gradient(135deg, #0D0D12 0%, #13131A 50%, #0D1117 100%);
    min-height: 100vh;
    padding: 32px 40px;
    display: flex;
    flex-direction: column;
    gap: 24px;
    color: #E8E8ED;
    font-family: 'Inter', system-ui, sans-serif;
  }

  .hero {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 24px;
    flex-wrap: wrap;
  }
  .hero h1 {
    font-family: 'Space Grotesk', system-ui, sans-serif;
    font-weight: 600;
    margin: 0 0 4px 0;
    font-size: 32px;
    background: linear-gradient(90deg, #00FF66, #00CCFF);
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
    letter-spacing: -0.02em;
  }
  .subtitle {
    color: #A0A0B0;
    font-size: 14px;
    max-width: 540px;
    line-height: 1.5;
    margin: 0;
  }

  .offline {
    display: flex;
    align-items: center;
  }
  .offline-card {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
    background: rgba(19, 19, 26, 0.72);
    border: 1px solid #2A2A3A;
    border-radius: 12px;
    backdrop-filter: blur(16px) saturate(120%);
  }
  .offline-card.fresh {
    border-color: rgba(0, 255, 102, 0.4);
    box-shadow: 0 0 16px rgba(0, 255, 102, 0.1);
  }
  .offline-card .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #00FF66;
    box-shadow: 0 0 6px #00FF66;
  }
  .offline-meta {
    display: flex;
    flex-direction: column;
    font-size: 12px;
    color: #A0A0B0;
  }
  .offline-meta strong {
    color: #00FF66;
    font-size: 14px;
  }
  .mono {
    font-family: 'JetBrains Mono', ui-monospace, monospace;
  }
  .sync {
    background: transparent;
    border: 1px solid rgba(0, 255, 102, 0.4);
    color: #00FF66;
    padding: 6px 12px;
    border-radius: 6px;
    cursor: pointer;
    font-family: 'JetBrains Mono', monospace;
    font-size: 11px;
    transition: all 0.2s;
  }
  .sync:hover:not(:disabled) {
    border-color: #00FF66;
    box-shadow: 0 0 8px rgba(0, 255, 102, 0.3);
  }
  .sync:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .tabs {
    display: flex;
    gap: 4px;
    border-bottom: 1px solid #2A2A3A;
    padding-bottom: 0;
  }
  .tab {
    background: transparent;
    border: none;
    color: #A0A0B0;
    padding: 10px 18px;
    cursor: pointer;
    font-family: 'Space Grotesk', sans-serif;
    font-weight: 500;
    font-size: 14px;
    border-bottom: 2px solid transparent;
    transition: all 0.2s;
    margin-bottom: -1px;
  }
  .tab:hover {
    color: #E8E8ED;
  }
  .tab.active {
    color: #00FF66;
    border-bottom-color: #00FF66;
  }
  .hint {
    color: #606070;
    font-family: 'Inter', sans-serif;
    font-size: 12px;
    margin: 0;
  }
  @media (prefers-reduced-motion: reduce) {
    .sync, .tab { transition: none; }
  }
</style>
