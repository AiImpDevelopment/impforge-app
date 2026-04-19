<!-- SPDX-License-Identifier: MIT -->
<!--
  HealthDashboard — circular gauges per server (latency p95 · success
  rate · calls/h · restart count).  ImpForge design rule: ALWAYS use
  circular gauges for stats, NEVER plain progress bars.

  Bottom panel: tamper-evident ledger badge + last 100 invocations
  drawer.
-->
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    health,
    refreshDashboard,
    refreshLedgerStatus,
    refreshLedgerRows,
    startPolling,
    stopPolling,
    startServer,
    stopServer,
  } from '$lib/stores/mcp/health.svelte';

  onMount(() => {
    startPolling();
    void refreshLedgerStatus();
    void refreshLedgerRows(null, 50);
  });
  onDestroy(() => {
    stopPolling();
  });

  let drawerOpen = $state(false);

  async function manualRefresh(): Promise<void> {
    await refreshDashboard();
    await refreshLedgerStatus();
    await refreshLedgerRows(null, 50);
  }

  function gaugeColour(rate: number): string {
    if (rate >= 0.99) return '#00FF66';
    if (rate >= 0.95) return '#00CCFF';
    if (rate >= 0.85) return '#FF8800';
    return '#FF3366';
  }
</script>

<section class="dashboard">
  <header class="actions">
    <button type="button" class="btn ghost" onclick={manualRefresh}>Refresh</button>
    <button type="button" class="btn ghost" onclick={() => (drawerOpen = !drawerOpen)}>
      {drawerOpen ? 'Hide ledger' : 'Show ledger'}
    </button>
  </header>

  {#if health.ledgerStatus}
    <div class="ledger-badge" class:ok={health.ledgerStatus.status === 'ok'} class:warn={health.ledgerStatus.status === 'tampered'}>
      <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
        <path
          d="M5 7V5a3 3 0 016 0v2h.5a1.5 1.5 0 011.5 1.5v5A1.5 1.5 0 0111.5 15h-7A1.5 1.5 0 013 13.5v-5A1.5 1.5 0 014.5 7H5z"
          fill="currentColor"
        />
      </svg>
      <span>
        {#if health.ledgerStatus.status === 'ok'}
          Tamper-evident · {health.ledgerStatus.rows} calls
        {:else if health.ledgerStatus.status === 'tampered'}
          Ledger tampered at seq #{health.ledgerStatus.broken_seq}
        {:else}
          No invocations yet
        {/if}
      </span>
    </div>
  {/if}

  {#if health.rows.length === 0}
    <div class="empty">
      <p>No installed servers — install one to see live health metrics.</p>
    </div>
  {:else}
    <div class="grid">
      {#each health.rows as row, idx (row.server.id)}
        <article class="card" style="animation-delay: {idx * 60}ms">
          <header class="card-head">
            <strong class="mono name">{row.server.id}</strong>
            <span class="status" class:on={row.running} class:off={!row.running}>
              {row.running ? 'running' : row.server.enabled ? 'stopped' : 'disabled'}
            </span>
          </header>

          <div class="gauges">
            <!-- Success rate -->
            <div class="gauge">
              <svg viewBox="0 0 100 100" width="64" height="64">
                <circle cx="50" cy="50" r="42" stroke="rgba(42, 42, 58, 0.6)" stroke-width="6" fill="none" />
                <circle
                  cx="50"
                  cy="50"
                  r="42"
                  stroke={gaugeColour(row.health.success_rate)}
                  stroke-width="6"
                  fill="none"
                  stroke-linecap="round"
                  stroke-dasharray={263.9}
                  stroke-dashoffset={263.9 * (1 - row.health.success_rate)}
                  transform="rotate(-90 50 50)"
                />
                <text x="50" y="55" text-anchor="middle" fill={gaugeColour(row.health.success_rate)} font-family="JetBrains Mono" font-size="18" font-weight="600">
                  {Math.round(row.health.success_rate * 100)}%
                </text>
              </svg>
              <span class="label">success rate</span>
            </div>

            <!-- Latency p95 -->
            <div class="gauge">
              <svg viewBox="0 0 100 100" width="64" height="64">
                <circle cx="50" cy="50" r="42" stroke="rgba(42, 42, 58, 0.6)" stroke-width="6" fill="none" />
                <circle
                  cx="50"
                  cy="50"
                  r="42"
                  stroke="#00CCFF"
                  stroke-width="6"
                  fill="none"
                  stroke-linecap="round"
                  stroke-dasharray={263.9}
                  stroke-dashoffset={263.9 * Math.max(0, 1 - Math.min((row.health.p95_latency_ms ?? 0) / 1000, 1))}
                  transform="rotate(-90 50 50)"
                />
                <text x="50" y="55" text-anchor="middle" fill="#00CCFF" font-family="JetBrains Mono" font-size="14" font-weight="600">
                  {row.health.p95_latency_ms ?? 0}ms
                </text>
              </svg>
              <span class="label">latency p95</span>
            </div>

            <!-- Restart count -->
            <div class="gauge">
              <svg viewBox="0 0 100 100" width="64" height="64">
                <circle cx="50" cy="50" r="42" stroke="rgba(42, 42, 58, 0.6)" stroke-width="6" fill="none" />
                <circle
                  cx="50"
                  cy="50"
                  r="42"
                  stroke={row.health.restart_count === 0 ? '#00FF66' : '#FF8800'}
                  stroke-width="6"
                  fill="none"
                  stroke-linecap="round"
                  stroke-dasharray={263.9}
                  stroke-dashoffset={263.9 * Math.max(0, 1 - Math.min(row.health.restart_count / 5, 1))}
                  transform="rotate(-90 50 50)"
                />
                <text x="50" y="55" text-anchor="middle" fill={row.health.restart_count === 0 ? '#00FF66' : '#FF8800'} font-family="JetBrains Mono" font-size="18" font-weight="600">
                  {row.health.restart_count}
                </text>
              </svg>
              <span class="label">restarts</span>
            </div>

            <!-- Uptime -->
            <div class="gauge">
              <svg viewBox="0 0 100 100" width="64" height="64">
                <circle cx="50" cy="50" r="42" stroke="rgba(42, 42, 58, 0.6)" stroke-width="6" fill="none" />
                <circle
                  cx="50"
                  cy="50"
                  r="42"
                  stroke="#9966FF"
                  stroke-width="6"
                  fill="none"
                  stroke-linecap="round"
                  stroke-dasharray={263.9}
                  stroke-dashoffset={263.9 * Math.max(0, 1 - Math.min(row.health.uptime_seconds / 3600, 1))}
                  transform="rotate(-90 50 50)"
                />
                <text x="50" y="55" text-anchor="middle" fill="#9966FF" font-family="JetBrains Mono" font-size="14" font-weight="600">
                  {Math.floor(row.health.uptime_seconds / 60)}m
                </text>
              </svg>
              <span class="label">uptime</span>
            </div>
          </div>

          <footer class="card-foot">
            {#if row.running}
              <button type="button" class="btn small" onclick={() => stopServer(row.server.id)}>Stop</button>
            {:else if row.server.enabled}
              <button type="button" class="btn small primary" onclick={() => startServer(row.server.id)}>Start</button>
            {/if}
          </footer>
        </article>
      {/each}
    </div>
  {/if}

  {#if drawerOpen}
    <aside class="drawer">
      <header>
        <h3>Last 50 invocations</h3>
      </header>
      {#if health.ledgerRows.length === 0}
        <p class="empty-text">No invocations recorded yet.</p>
      {:else}
        <ul>
          {#each health.ledgerRows as row (row.seq)}
            <li class:success={row.success} class:failure={!row.success}>
              <span class="seq mono">#{row.seq}</span>
              <span class="ts mono">{new Date(row.timestamp).toLocaleTimeString()}</span>
              <span class="srv mono">{row.server_id}</span>
              <span class="tool mono">{row.tool_name}</span>
              <span class="latency mono">{row.latency_ms}ms</span>
              <span class="hash mono" title="{row.row_hash}">…{row.row_hash.slice(-6)}</span>
            </li>
          {/each}
        </ul>
      {/if}
    </aside>
  {/if}
</section>

<style>
  .dashboard {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  .btn {
    background: transparent;
    border: 1px solid #2A2A3A;
    color: #A0A0B0;
    padding: 7px 12px;
    border-radius: 8px;
    font-family: 'JetBrains Mono', monospace;
    font-size: 11px;
    cursor: pointer;
    transition: all 0.2s;
  }
  .btn:hover {
    color: #E8E8ED;
    border-color: #A0A0B0;
  }
  .btn.ghost { background: transparent; }
  .btn.small { padding: 4px 10px; }
  .btn.primary {
    background: linear-gradient(135deg, #00FF66 0%, #00CC52 100%);
    color: #0D0D12;
    border-color: #00FF66;
  }
  .ledger-badge {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 8px 14px;
    background: rgba(19, 19, 26, 0.72);
    border: 1px solid #2A2A3A;
    border-radius: 999px;
    color: #A0A0B0;
    font-family: 'JetBrains Mono', monospace;
    font-size: 12px;
    width: fit-content;
  }
  .ledger-badge.ok {
    border-color: rgba(0, 255, 102, 0.4);
    color: #00FF66;
    box-shadow: 0 0 8px rgba(0, 255, 102, 0.2);
  }
  .ledger-badge.warn {
    border-color: rgba(255, 136, 0, 0.4);
    color: #FF8800;
    box-shadow: 0 0 8px rgba(255, 136, 0, 0.2);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 14px;
  }
  .card {
    background: rgba(19, 19, 26, 0.72);
    border: 1px solid #2A2A3A;
    border-radius: 14px;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    backdrop-filter: blur(16px) saturate(120%);
    animation: cardEnter 0.4s cubic-bezier(0.16, 1, 0.3, 1) backwards;
    transition: all 0.2s;
  }
  .card:hover {
    border-color: rgba(0, 255, 102, 0.4);
    box-shadow: 0 0 16px rgba(0, 255, 102, 0.1);
  }
  .card-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .name {
    color: #00FF66;
    font-size: 14px;
    font-weight: 600;
  }
  .status {
    font-size: 11px;
    font-family: 'JetBrains Mono', monospace;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 2px 8px;
    border-radius: 999px;
  }
  .status.on {
    background: rgba(0, 255, 102, 0.12);
    color: #00FF66;
  }
  .status.off {
    background: rgba(96, 96, 112, 0.16);
    color: #A0A0B0;
  }
  .gauges {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 12px;
  }
  .gauge {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }
  .gauge .label {
    font-family: 'Inter', sans-serif;
    font-size: 11px;
    color: #606070;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .card-foot {
    display: flex;
    justify-content: flex-end;
  }
  .empty {
    text-align: center;
    padding: 40px;
    color: #A0A0B0;
  }
  .drawer {
    background: rgba(8, 8, 13, 0.85);
    border: 1px solid #2A2A3A;
    border-radius: 12px;
    padding: 16px;
    backdrop-filter: blur(16px) saturate(120%);
  }
  .drawer header h3 {
    font-family: 'Space Grotesk', sans-serif;
    font-size: 13px;
    color: #E8E8ED;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin: 0 0 10px 0;
  }
  .empty-text {
    color: #606070;
    margin: 0;
  }
  ul {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 11px;
    max-height: 320px;
    overflow-y: auto;
  }
  li {
    display: grid;
    grid-template-columns: 50px 80px 1fr 1.4fr 60px 70px;
    gap: 10px;
    padding: 4px 8px;
    border-radius: 4px;
    align-items: center;
  }
  li.success { background: rgba(0, 255, 102, 0.04); }
  li.failure { background: rgba(255, 51, 102, 0.06); }
  .seq { color: #606070; }
  .ts { color: #A0A0B0; }
  .srv { color: #00CCFF; }
  .tool { color: #E8E8ED; }
  .latency { color: #606070; text-align: right; }
  .hash { color: #9966FF; text-align: right; cursor: help; }
  .mono {
    font-family: 'JetBrains Mono', ui-monospace, monospace;
  }
  @keyframes cardEnter {
    from { opacity: 0; transform: translateY(8px); }
    to { opacity: 1; transform: translateY(0); }
  }
  @media (prefers-reduced-motion: reduce) {
    .card { animation: none; transition: none; }
    .btn { transition: none; }
  }
</style>
