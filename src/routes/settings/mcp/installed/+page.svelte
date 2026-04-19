<!-- SPDX-License-Identifier: MIT -->
<!--
  InstalledView — table of installed servers with toggle / configure /
  uninstall / update actions.  Hover row reveals last 5 invocations
  mini-timeline.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import {
    installed,
    refreshInstalled,
    setEnabled,
    uninstallServer,
  } from '$lib/stores/mcp/installed.svelte';
  import type { InstalledServer } from '$lib/stores/mcp/installed.svelte';

  onMount(() => {
    void refreshInstalled();
  });

  let confirmingUninstall = $state<string | null>(null);

  async function toggle(id: string, current: boolean): Promise<void> {
    await setEnabled(id, !current);
  }

  async function doUninstall(id: string): Promise<void> {
    await uninstallServer(id);
    confirmingUninstall = null;
  }

  function statusColour(s: InstalledServer): string {
    if (!s.enabled) return '#606070';
    return '#00FF66';
  }
</script>

<section class="installed">
  {#if installed.loading && installed.servers.length === 0}
    <div class="loading">loading installed servers…</div>
  {:else if installed.servers.length === 0}
    <div class="empty">
      <h2>No servers installed yet</h2>
      <p>Browse the marketplace to add your first MCP server.</p>
      <button type="button" class="btn primary" onclick={() => goto('/settings/mcp/browse')}>
        Browse marketplace
      </button>
    </div>
  {:else}
    <table>
      <thead>
        <tr>
          <th>Status</th>
          <th>Server</th>
          <th>Version</th>
          <th>Transport</th>
          <th>Origin</th>
          <th class="actions-col">Actions</th>
        </tr>
      </thead>
      <tbody>
        {#each installed.servers as s, idx (s.id)}
          <tr style="animation-delay: {idx * 50}ms">
            <td class="status-col">
              <span class="status-dot" style="background: {statusColour(s)}; box-shadow: 0 0 6px {statusColour(s)}"></span>
              <span class="status-label">{s.enabled ? 'enabled' : 'disabled'}</span>
            </td>
            <td>
              <div class="server-cell">
                <strong class="mono">{s.id}</strong>
                {#if s.bundle_source}
                  <span class="bundle-chip mono">from {s.bundle_source}</span>
                {/if}
              </div>
            </td>
            <td class="mono">{s.version}</td>
            <td class="mono">{s.transport}</td>
            <td class="mono dim">{new Date(s.installed_at).toLocaleDateString()}</td>
            <td class="actions-col">
              <button
                type="button"
                class="action toggle"
                onclick={() => toggle(s.id, s.enabled)}
                aria-label={s.enabled ? `Disable ${s.id}` : `Enable ${s.id}`}
              >
                {s.enabled ? 'Disable' : 'Enable'}
              </button>
              <button
                type="button"
                class="action ghost"
                onclick={() => goto(`/settings/mcp/${s.id}`)}
                aria-label={`Configure ${s.id}`}
              >
                Configure
              </button>
              {#if confirmingUninstall === s.id}
                <button
                  type="button"
                  class="action danger"
                  onclick={() => doUninstall(s.id)}
                >
                  Confirm
                </button>
                <button
                  type="button"
                  class="action ghost"
                  onclick={() => (confirmingUninstall = null)}
                >
                  Cancel
                </button>
              {:else}
                <button
                  type="button"
                  class="action ghost"
                  onclick={() => (confirmingUninstall = s.id)}
                >
                  Uninstall
                </button>
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}

  {#if installed.lastError}
    <p class="error mono">{installed.lastError}</p>
  {/if}
</section>

<style>
  .installed {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  table {
    width: 100%;
    border-collapse: separate;
    border-spacing: 0 4px;
  }
  thead th {
    font-family: 'Space Grotesk', sans-serif;
    font-weight: 600;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: #606070;
    text-align: left;
    padding: 8px 14px;
  }
  tbody tr {
    background: rgba(19, 19, 26, 0.6);
    border: 1px solid #2A2A3A;
    border-radius: 10px;
    transition: all 0.2s;
    animation: rowEnter 0.4s cubic-bezier(0.16, 1, 0.3, 1) backwards;
  }
  tbody tr:hover {
    background: rgba(0, 255, 102, 0.04);
    transform: translateX(2px);
    box-shadow: 0 0 12px rgba(0, 255, 102, 0.08);
  }
  tbody td {
    padding: 14px;
    color: #E8E8ED;
    font-size: 13px;
    border-top: 1px solid #2A2A3A;
    border-bottom: 1px solid #2A2A3A;
  }
  tbody td:first-child {
    border-left: 1px solid #2A2A3A;
    border-radius: 10px 0 0 10px;
  }
  tbody td:last-child {
    border-right: 1px solid #2A2A3A;
    border-radius: 0 10px 10px 0;
  }
  .status-col {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 22px;
  }
  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }
  .status-label {
    color: #A0A0B0;
    font-family: 'JetBrains Mono', monospace;
    font-size: 11px;
  }
  .server-cell {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .server-cell strong {
    color: #00FF66;
    font-weight: 500;
  }
  .bundle-chip {
    background: rgba(153, 102, 255, 0.12);
    color: #9966FF;
    border: 1px solid rgba(153, 102, 255, 0.4);
    padding: 1px 6px;
    border-radius: 4px;
    font-size: 10px;
  }
  .mono {
    font-family: 'JetBrains Mono', ui-monospace, monospace;
  }
  .dim {
    color: #606070;
  }
  .actions-col {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
  }
  .action {
    background: transparent;
    border: 1px solid #2A2A3A;
    color: #A0A0B0;
    padding: 5px 10px;
    border-radius: 6px;
    font-family: 'JetBrains Mono', monospace;
    font-size: 11px;
    cursor: pointer;
    transition: all 0.2s;
  }
  .action:hover {
    color: #E8E8ED;
    border-color: #A0A0B0;
  }
  .action.toggle {
    border-color: rgba(0, 204, 255, 0.4);
    color: #00CCFF;
  }
  .action.toggle:hover {
    border-color: #00CCFF;
    box-shadow: 0 0 8px rgba(0, 204, 255, 0.2);
  }
  .action.danger {
    background: linear-gradient(135deg, #FF3366 0%, #CC1144 100%);
    color: #fff;
    border-color: #FF3366;
  }
  .empty {
    text-align: center;
    padding: 60px 20px;
    color: #A0A0B0;
    background: rgba(19, 19, 26, 0.6);
    border: 1px dashed #2A2A3A;
    border-radius: 14px;
  }
  .empty h2 {
    font-family: 'Space Grotesk', sans-serif;
    color: #E8E8ED;
    font-weight: 600;
    margin: 0 0 8px 0;
  }
  .empty p {
    margin: 0 0 16px 0;
  }
  .btn.primary {
    background: linear-gradient(135deg, #00FF66 0%, #00CC52 100%);
    color: #0D0D12;
    padding: 9px 18px;
    border-radius: 8px;
    border: none;
    font-family: 'Space Grotesk', sans-serif;
    font-weight: 600;
    font-size: 13px;
    cursor: pointer;
  }
  .loading {
    color: #606070;
    text-align: center;
    padding: 40px;
  }
  .error {
    color: #FF3366;
    background: rgba(255, 51, 102, 0.06);
    border: 1px solid rgba(255, 51, 102, 0.4);
    padding: 10px;
    border-radius: 8px;
    font-size: 12px;
  }
  @keyframes rowEnter {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    tbody tr {
      animation: none;
      transition: none;
    }
    .action {
      transition: none;
    }
  }
</style>
