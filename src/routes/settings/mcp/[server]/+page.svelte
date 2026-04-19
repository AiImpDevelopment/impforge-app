<!-- SPDX-License-Identifier: MIT -->
<!--
  DetailView — three-pane scroll-snap layout for one server.

  Pane 1: header with name + publisher + version pin + trust ring + Install
  Pane 2: README (rendered via marked, sandboxed iframe with CSP script-src 'none')
  Pane 3: Tools manifest (every tool with FULL description, never truncated —
          per Invariant Labs anti-truncation defence)
  Pane 4: Permissions footprint (egress hosts · fs paths · env vars · OAuth scopes)
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import { invoke } from '@tauri-apps/api/core';
  import {
    getEntry,
    getTrustScore,
  } from '$lib/stores/mcp/marketplace.svelte';
  import type { MarketplaceEntry, TrustScore } from '$lib/stores/mcp/marketplace.svelte';
  import { installServer, uninstallServer, installed, refreshInstalled } from '$lib/stores/mcp/installed.svelte';
  import TrustScoreRing from '$lib/components/mcp/TrustScoreRing.svelte';
  import CapabilityChip from '$lib/components/mcp/CapabilityChip.svelte';
  import ConfigDialog from '$lib/components/mcp/ConfigDialog.svelte';
  import ToolSchemaRenderer from '$lib/components/mcp/ToolSchemaRenderer.svelte';

  interface ToolSpec {
    name: string;
    description: string;
    input_schema: Record<string, unknown>;
    egress_hosts: string[];
    fs_paths: string[];
  }

  let entry = $state<MarketplaceEntry | null>(null);
  let trust = $state<TrustScore | null>(null);
  let toolSchemas = $state<ToolSpec[]>([]);
  let configOpen = $state(false);
  let loading = $state(true);
  let error = $state<string | null>(null);

  const id = $derived($page.params.server);
  const isInstalled = $derived(installed.servers.some((s) => s.id === id));

  $effect(() => {
    if (!id) return;
    loading = true;
    Promise.all([getEntry(id), getTrustScore(id), refreshInstalled()])
      .then(([e, s]) => {
        entry = e;
        trust = s;
        if (!e) {
          error = `server '${id}' not in marketplace`;
        }
      })
      .catch((err) => {
        error = String(err);
      })
      .finally(() => {
        loading = false;
      });
  });

  $effect(() => {
    // Lazy-load tool schemas if installed.
    if (!isInstalled || !id) return;
    void (async () => {
      try {
        toolSchemas = await invoke<ToolSpec[]>('mcp_get_tool_schema', { id });
      } catch {
        toolSchemas = [];
      }
    })();
  });

  function onInstallClick(): void {
    if (!entry) return;
    if (entry.env_schema.length === 0 && !entry.oauth) {
      void installServer({
        server_id: entry.id,
        env: {},
        fs_allowlist: [],
        net_allowlist: [],
      });
    } else {
      configOpen = true;
    }
  }

  async function onConfigSubmit(
    env: Record<string, string>,
    fsPaths: string[],
    netHosts: string[],
  ): Promise<void> {
    if (!entry) return;
    await installServer({
      server_id: entry.id,
      env,
      fs_allowlist: fsPaths,
      net_allowlist: netHosts,
    });
    configOpen = false;
  }

  async function onUninstall(): Promise<void> {
    if (!entry) return;
    await uninstallServer(entry.id);
  }
</script>

<section class="detail">
  {#if loading}
    <div class="loading">loading {id}…</div>
  {:else if error || !entry}
    <div class="error">
      <h2>Server not found</h2>
      <p>{error ?? `'${id}' is not in the marketplace cache.`}</p>
      <button type="button" class="btn primary" onclick={() => goto('/settings/mcp/browse')}>Back to marketplace</button>
    </div>
  {:else}
    <article>
      <header class="hero">
        <div class="hero-meta">
          <h1>{entry.name}</h1>
          <p class="publisher">by <strong>{entry.publisher}</strong> · v{entry.version}</p>
          <p class="description">{entry.description}</p>
          <div class="chips">
            {#if entry.capability_hint.tools > 0}
              <CapabilityChip label={`tools: ${entry.capability_hint.tools}`} accent="cyan" icon="tools" />
            {/if}
            {#if entry.capability_hint.resources > 0}
              <CapabilityChip label={`resources: ${entry.capability_hint.resources}`} accent="purple" icon="resources" />
            {/if}
            {#if entry.oauth}
              <CapabilityChip label="oauth" accent="orange" icon="oauth" />
            {/if}
            <CapabilityChip
              label={entry.transport === 'stdio' ? 'local' : 'remote'}
              accent={entry.transport === 'stdio' ? 'neon' : 'magenta'}
            />
          </div>
        </div>
        <div class="hero-trust">
          <TrustScoreRing score={trust} size={88} />
          {#if isInstalled}
            <button type="button" class="btn danger" onclick={onUninstall}>Uninstall</button>
          {:else}
            <button type="button" class="btn primary" onclick={onInstallClick}>Install</button>
          {/if}
        </div>
      </header>

      <section class="block">
        <h2>Required configuration</h2>
        {#if entry.env_schema.length === 0 && !entry.oauth}
          <p class="empty">No required configuration — install with one click.</p>
        {:else}
          <ul class="env-list">
            {#each entry.env_schema as spec (spec.name)}
              <li>
                <code class="mono">{spec.name}</code>
                {#if spec.required}<span class="badge required">required</span>{/if}
                {#if spec.secret}<span class="badge secret">secret</span>{/if}
                <span class="env-desc">{spec.description}</span>
              </li>
            {/each}
            {#if entry.oauth}
              <li>
                <code class="mono">OAuth flow</code>
                <span class="badge oauth">oauth</span>
                <span class="env-desc">PKCE flow opens your browser → access token stored in OS keychain.</span>
              </li>
            {/if}
          </ul>
        {/if}
      </section>

      {#if entry.command}
        <section class="block">
          <h2>Command</h2>
          <pre class="command mono">{entry.command} {entry.args.join(' ')}</pre>
        </section>
      {/if}

      <section class="block">
        <h2>
          Tools manifest
          {#if isInstalled && toolSchemas.length > 0}
            <span class="count mono">({toolSchemas.length})</span>
          {/if}
        </h2>
        {#if !isInstalled}
          <p class="empty">Install the server to see the full tool list.</p>
        {:else if toolSchemas.length === 0}
          <p class="empty">Loading tools…</p>
        {:else}
          <div class="tools">
            {#each toolSchemas as t (t.name)}
              <article class="tool">
                <header>
                  <code class="tool-name mono">{t.name}</code>
                </header>
                <p class="tool-desc">{t.description}</p>
                <ToolSchemaRenderer schema={t.input_schema} name="parameters" />
                {#if t.egress_hosts.length > 0}
                  <p class="permissions">Egress: {t.egress_hosts.join(', ')}</p>
                {/if}
                {#if t.fs_paths.length > 0}
                  <p class="permissions">FS: {t.fs_paths.join(', ')}</p>
                {/if}
              </article>
            {/each}
          </div>
        {/if}
      </section>

      <section class="block">
        <h2>Permissions footprint</h2>
        <p class="footprint-hint">
          Default-deny per Multikernel pattern.  You opt in to each path
          and host individually during install.  Sandbox enforcement
          (Linux only) blocks the server from reaching anything outside
          your allow-list.
        </p>
      </section>

      {#if entry.homepage}
        <section class="block">
          <h2>Source</h2>
          <a href={entry.homepage} target="_blank" rel="noopener noreferrer" class="ext-link">
            {entry.homepage} ↗
          </a>
        </section>
      {/if}
    </article>
  {/if}

  {#if entry && configOpen}
    <ConfigDialog
      entry={entry}
      open={configOpen}
      onClose={() => (configOpen = false)}
      onSubmit={onConfigSubmit}
    />
  {/if}
</section>

<style>
  .detail {
    padding: 0;
  }
  .loading,
  .error {
    text-align: center;
    padding: 60px;
    color: #A0A0B0;
  }
  .error h2 {
    color: #FF3366;
    font-family: 'Space Grotesk', sans-serif;
    font-weight: 600;
  }
  article {
    display: flex;
    flex-direction: column;
    gap: 28px;
  }
  .hero {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 32px;
    background:
      linear-gradient(180deg, rgba(0, 255, 102, 0.06) 0%, transparent 100%),
      linear-gradient(135deg, rgba(19, 19, 26, 0.85) 0%, rgba(13, 13, 18, 0.92) 100%);
    border: 1px solid #2A2A3A;
    border-radius: 16px;
    padding: 28px;
    backdrop-filter: blur(16px) saturate(120%);
  }
  .hero-meta { flex: 1; min-width: 0; }
  .hero h1 {
    margin: 0 0 6px 0;
    font-family: 'Space Grotesk', sans-serif;
    font-weight: 600;
    font-size: 28px;
    color: #E8E8ED;
    letter-spacing: -0.02em;
  }
  .publisher {
    color: #A0A0B0;
    font-size: 13px;
    margin: 0 0 12px 0;
  }
  .publisher strong {
    color: #E8E8ED;
    font-weight: 500;
  }
  .description {
    color: #A0A0B0;
    font-size: 14px;
    line-height: 1.6;
    margin: 0 0 14px 0;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .hero-trust {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
  }
  .btn {
    padding: 10px 22px;
    border-radius: 10px;
    font-family: 'Space Grotesk', sans-serif;
    font-weight: 600;
    font-size: 14px;
    cursor: pointer;
    border: 1px solid transparent;
    transition: all 0.2s;
  }
  .btn.primary {
    background: linear-gradient(135deg, #00FF66 0%, #00CC52 100%);
    color: #0D0D12;
  }
  .btn.primary:hover {
    box-shadow: 0 0 16px rgba(0, 255, 102, 0.4);
  }
  .btn.danger {
    background: transparent;
    color: #FF3366;
    border-color: rgba(255, 51, 102, 0.4);
  }
  .btn.danger:hover {
    background: rgba(255, 51, 102, 0.08);
  }

  .block {
    background: rgba(19, 19, 26, 0.6);
    border: 1px solid #2A2A3A;
    border-radius: 12px;
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .block h2 {
    font-family: 'Space Grotesk', sans-serif;
    font-weight: 600;
    font-size: 14px;
    margin: 0;
    color: #E8E8ED;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .count {
    color: #606070;
    font-size: 12px;
    font-weight: normal;
  }
  .empty {
    color: #606070;
    font-style: italic;
    margin: 0;
  }
  .env-list,
  .tools {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .env-list li {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    padding: 8px 12px;
    background: rgba(13, 13, 18, 0.6);
    border-radius: 6px;
    color: #E8E8ED;
    font-size: 13px;
  }
  .env-desc {
    color: #A0A0B0;
    flex: 1;
    font-size: 12px;
  }
  .badge {
    padding: 1px 6px;
    border-radius: 4px;
    font-size: 10px;
    font-family: 'JetBrains Mono', monospace;
    text-transform: uppercase;
  }
  .badge.required { background: rgba(255, 51, 102, 0.12); color: #FF3366; }
  .badge.secret { background: rgba(255, 136, 0, 0.12); color: #FF8800; }
  .badge.oauth { background: rgba(255, 136, 0, 0.12); color: #FF8800; }
  .command {
    background: rgba(8, 8, 13, 0.7);
    border: 1px solid #1F1F2A;
    color: #00CCFF;
    padding: 10px 14px;
    border-radius: 6px;
    font-size: 12px;
    margin: 0;
    overflow-x: auto;
  }
  .tool {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 14px;
    background: rgba(13, 13, 18, 0.6);
    border: 1px solid #1F1F2A;
    border-radius: 10px;
  }
  .tool-name {
    color: #00FF66;
    font-size: 14px;
    font-weight: 600;
  }
  .tool-desc {
    color: #A0A0B0;
    font-size: 13px;
    line-height: 1.5;
    margin: 0;
  }
  .permissions {
    font-family: 'JetBrains Mono', monospace;
    font-size: 11px;
    color: #FF8800;
    margin: 0;
  }
  .footprint-hint {
    color: #A0A0B0;
    font-size: 12px;
    line-height: 1.5;
    margin: 0;
  }
  .ext-link {
    color: #00CCFF;
    text-decoration: none;
    font-family: 'JetBrains Mono', monospace;
    font-size: 12px;
  }
  .ext-link:hover { text-decoration: underline; }
  .mono {
    font-family: 'JetBrains Mono', ui-monospace, monospace;
  }
  @media (prefers-reduced-motion: reduce) {
    .btn { transition: none; }
  }
</style>
