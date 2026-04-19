<!-- SPDX-License-Identifier: MIT -->
<!--
  BrowseView — bento marketplace grid.  Layout obeys ImpForge design
  rules:
    * search bar with neon focus ring
    * left rail of category chips (magenta active)
    * asymmetric bento grid (NOT uniform 3-col)
    * stagger entry animation per card
    * trust filter row
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import {
    marketplace,
    refresh,
    setQuery,
    setCategory,
    toggleVerified,
    toggleSigned,
    toggleNoOauth,
  } from '$lib/stores/mcp/marketplace.svelte';
  import type { Category, MarketplaceEntry } from '$lib/stores/mcp/marketplace.svelte';
  import { installServer, refreshInstalled } from '$lib/stores/mcp/installed.svelte';
  import ServerCard from '$lib/components/mcp/ServerCard.svelte';
  import ConfigDialog from '$lib/components/mcp/ConfigDialog.svelte';
  import BundleInstallWizard from '$lib/components/mcp/BundleInstallWizard.svelte';

  const CATEGORIES: { value: Category; label: string }[] = [
    { value: 'files', label: 'Files' },
    { value: 'web', label: 'Web' },
    { value: 'comms', label: 'Comms' },
    { value: 'code', label: 'Code' },
    { value: 'data', label: 'Data' },
    { value: 'ai', label: 'AI' },
    { value: 'custom', label: 'Custom' },
  ];

  let queryInput = $state('');
  let configEntry = $state<MarketplaceEntry | null>(null);
  let bundleWizardOpen = $state(false);
  let snackbar = $state<string | null>(null);

  onMount(() => {
    void refresh();
    void refreshInstalled();
  });

  async function onQueryInput(): Promise<void> {
    setQuery(queryInput);
    await refresh();
  }

  async function onCategoryClick(c: Category | null): Promise<void> {
    setCategory(c);
    await refresh();
  }

  async function onInstallClick(id: string): Promise<void> {
    const entry = marketplace.entries.find((e) => e.id === id);
    if (!entry) return;
    if (entry.env_schema.length === 0 && !entry.oauth) {
      // No required config — install instantly.
      const result = await installServer({
        server_id: id,
        env: {},
        fs_allowlist: [],
        net_allowlist: [],
      });
      snackbar = result ? `Installed ${entry.name}` : `Failed: ${marketplace.lastError ?? 'unknown error'}`;
      setTimeout(() => (snackbar = null), 3000);
    } else {
      configEntry = entry;
    }
  }

  function onDetailsClick(id: string): void {
    goto(`/settings/mcp/${id}`);
  }

  async function onConfigSubmit(
    env: Record<string, string>,
    fsPaths: string[],
    netHosts: string[],
  ): Promise<void> {
    if (!configEntry) return;
    const result = await installServer({
      server_id: configEntry.id,
      env,
      fs_allowlist: fsPaths,
      net_allowlist: netHosts,
    });
    snackbar = result ? `Installed ${configEntry.name}` : `Failed: ${marketplace.lastError ?? 'unknown error'}`;
    setTimeout(() => (snackbar = null), 3000);
    configEntry = null;
  }

  // Sort entries into bento "weights" — 1st, 6th, 11th = wide; rest = standard.
  const grouped = $derived.by(() => {
    return marketplace.entries.map((entry, idx) => ({
      entry,
      weight: idx % 5 === 0 ? 'wide' : 'standard',
      index: idx,
    }));
  });
</script>

<section class="browse">
  <div class="layout">
    <aside class="rail">
      <header>
        <h2>Filter</h2>
      </header>
      <ul class="categories">
        <li>
          <button
            type="button"
            class="cat"
            class:active={marketplace.category === null}
            onclick={() => onCategoryClick(null)}
          >
            <span class="dot" aria-hidden="true"></span>
            All
          </button>
        </li>
        {#each CATEGORIES as c (c.value)}
          <li>
            <button
              type="button"
              class="cat"
              class:active={marketplace.category === c.value}
              onclick={() => onCategoryClick(c.value)}
            >
              <span class="dot" aria-hidden="true"></span>
              {c.label}
            </button>
          </li>
        {/each}
      </ul>

      <header>
        <h2>Trust</h2>
      </header>
      <div class="toggle-list">
        <label class="toggle">
          <input
            type="checkbox"
            checked={marketplace.verifiedOnly}
            onchange={async () => {
              toggleVerified();
              await refresh();
            }}
          />
          <span>Verified only</span>
        </label>
        <label class="toggle">
          <input
            type="checkbox"
            checked={marketplace.signedOnly}
            onchange={async () => {
              toggleSigned();
              await refresh();
            }}
          />
          <span>Signed only</span>
        </label>
        <label class="toggle">
          <input
            type="checkbox"
            checked={marketplace.noOauth}
            onchange={async () => {
              toggleNoOauth();
              await refresh();
            }}
          />
          <span>No OAuth</span>
        </label>
      </div>

      <button type="button" class="bundle-cta" onclick={() => (bundleWizardOpen = true)}>
        ⚡ Quick-start bundle
      </button>
    </aside>

    <div class="main">
      <div class="search-row">
        <div class="search-wrap">
          <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
            <circle cx="7" cy="7" r="5" fill="none" stroke="currentColor" stroke-width="1.5" />
            <path d="M11 11l3 3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
          </svg>
          <input
            type="search"
            class="search"
            placeholder="search 6 000+ MCP servers…"
            bind:value={queryInput}
            oninput={onQueryInput}
          />
        </div>
        <span class="result-count mono">{marketplace.entries.length} results</span>
      </div>

      {#if marketplace.loading}
        <div class="loading">
          <div class="bar"></div>
          <p>loading marketplace…</p>
        </div>
      {:else if marketplace.entries.length === 0}
        <div class="empty">
          <p>No servers match.  Try clearing filters or syncing the mirror.</p>
        </div>
      {:else}
        <div class="grid">
          {#each grouped as { entry, weight, index } (entry.id)}
            <div class="cell" class:wide={weight === 'wide'}>
              <ServerCard {entry} {index} onInstall={onInstallClick} onDetails={onDetailsClick} />
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </div>

  {#if configEntry}
    <ConfigDialog
      entry={configEntry}
      open={true}
      onClose={() => (configEntry = null)}
      onSubmit={onConfigSubmit}
    />
  {/if}

  {#if bundleWizardOpen}
    <div class="bundle-overlay" onclick={() => (bundleWizardOpen = false)} role="presentation">
      <div onclick={(e) => e.stopPropagation()} class="bundle-host" role="presentation">
        <BundleInstallWizard
          onClose={() => (bundleWizardOpen = false)}
          onComplete={() => {
            void refreshInstalled();
            snackbar = 'Bundle installed';
            setTimeout(() => (snackbar = null), 3000);
          }}
        />
      </div>
    </div>
  {/if}

  {#if snackbar}
    <div class="snackbar mono">{snackbar}</div>
  {/if}
</section>

<style>
  .browse {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .layout {
    display: grid;
    grid-template-columns: 240px 1fr;
    gap: 24px;
    align-items: start;
  }
  .rail {
    background: rgba(19, 19, 26, 0.72);
    border: 1px solid #2A2A3A;
    border-radius: 14px;
    padding: 18px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    backdrop-filter: blur(16px) saturate(120%);
    position: sticky;
    top: 16px;
  }
  .rail h2 {
    margin: 0;
    font-family: 'Space Grotesk', sans-serif;
    font-weight: 600;
    font-size: 11px;
    color: #606070;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .categories {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .cat {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    background: transparent;
    border: 1px solid transparent;
    color: #A0A0B0;
    padding: 6px 10px;
    border-radius: 8px;
    cursor: pointer;
    font-family: 'Space Grotesk', sans-serif;
    font-size: 13px;
    text-align: left;
    transition: all 0.2s;
  }
  .cat .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #2A2A3A;
  }
  .cat:hover {
    color: #E8E8ED;
    background: rgba(255, 255, 255, 0.03);
  }
  .cat.active {
    color: #FF3399;
    background: rgba(255, 51, 153, 0.08);
    border-color: rgba(255, 51, 153, 0.3);
  }
  .cat.active .dot {
    background: #FF3399;
    box-shadow: 0 0 4px #FF3399;
  }

  .toggle-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: 8px;
    color: #A0A0B0;
    font-family: 'Inter', sans-serif;
    font-size: 12px;
    cursor: pointer;
  }
  .toggle input[type='checkbox'] {
    accent-color: #00CCFF;
  }
  .toggle:hover { color: #E8E8ED; }

  .bundle-cta {
    background: linear-gradient(135deg, rgba(153, 102, 255, 0.16) 0%, rgba(255, 51, 153, 0.08) 100%);
    border: 1px solid rgba(153, 102, 255, 0.4);
    color: #9966FF;
    padding: 10px;
    border-radius: 10px;
    cursor: pointer;
    font-family: 'Space Grotesk', sans-serif;
    font-weight: 600;
    font-size: 13px;
    transition: all 0.2s;
    margin-top: 4px;
  }
  .bundle-cta:hover {
    box-shadow: 0 0 16px rgba(153, 102, 255, 0.3);
    border-color: #9966FF;
  }

  .main {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .search-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .search-wrap {
    flex: 1;
    position: relative;
    color: #606070;
  }
  .search-wrap svg {
    position: absolute;
    left: 12px;
    top: 50%;
    transform: translateY(-50%);
  }
  .search {
    width: 100%;
    background: rgba(8, 8, 13, 0.72);
    border: 1px solid #2A2A3A;
    color: #E8E8ED;
    padding: 12px 14px 12px 36px;
    border-radius: 10px;
    font-family: 'Inter', sans-serif;
    font-size: 14px;
    transition: all 0.2s;
    box-sizing: border-box;
  }
  .search:focus {
    outline: none;
    border-color: #00FF66;
    box-shadow: 0 0 12px rgba(0, 255, 102, 0.2);
    color: #E8E8ED;
  }
  .result-count {
    color: #606070;
    font-size: 12px;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 16px;
  }
  .cell.wide {
    grid-column: span 2;
  }
  @media (max-width: 1024px) {
    .layout {
      grid-template-columns: 1fr;
    }
    .grid {
      grid-template-columns: repeat(2, 1fr);
    }
    .cell.wide {
      grid-column: span 2;
    }
    .rail {
      position: static;
    }
  }
  @media (max-width: 640px) {
    .grid {
      grid-template-columns: 1fr;
    }
    .cell.wide {
      grid-column: auto;
    }
  }

  .loading {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    padding: 40px;
    color: #A0A0B0;
  }
  .loading .bar {
    width: 200px;
    height: 3px;
    background: linear-gradient(90deg, transparent, #00FF66, transparent);
    background-size: 200% 100%;
    animation: scan 1.4s linear infinite;
    border-radius: 999px;
  }
  .empty {
    text-align: center;
    padding: 40px;
    color: #A0A0B0;
  }
  @keyframes scan {
    from { background-position: -100% 0; }
    to { background-position: 200% 0; }
  }

  .snackbar {
    position: fixed;
    bottom: 24px;
    left: 50%;
    transform: translateX(-50%);
    background: linear-gradient(135deg, rgba(0, 255, 102, 0.15) 0%, rgba(19, 19, 26, 0.95) 100%);
    border: 1px solid rgba(0, 255, 102, 0.4);
    color: #00FF66;
    padding: 12px 24px;
    border-radius: 10px;
    box-shadow: 0 0 24px rgba(0, 255, 102, 0.2);
    backdrop-filter: blur(16px);
    z-index: 200;
    font-size: 13px;
    animation: slideUp 0.3s cubic-bezier(0.16, 1, 0.3, 1);
  }
  @keyframes slideUp {
    from { opacity: 0; transform: translate(-50%, 12px); }
    to { opacity: 1; transform: translate(-50%, 0); }
  }

  .bundle-overlay {
    position: fixed;
    inset: 0;
    background: rgba(8, 8, 13, 0.78);
    backdrop-filter: blur(8px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    animation: fade 0.2s ease;
  }
  .bundle-host {
    width: min(720px, 92vw);
    max-height: 88vh;
    overflow-y: auto;
  }
  @keyframes fade {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  .mono {
    font-family: 'JetBrains Mono', ui-monospace, monospace;
  }

  @media (prefers-reduced-motion: reduce) {
    .loading .bar,
    .snackbar,
    .bundle-overlay {
      animation: none;
    }
    .cat,
    .bundle-cta,
    .search,
    .toggle {
      transition: none;
    }
  }
</style>
