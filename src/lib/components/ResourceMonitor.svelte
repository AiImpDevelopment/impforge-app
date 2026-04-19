<!-- SPDX-License-Identifier: MIT -->
<!-- ResourceMonitor.svelte — circular gauges (per design rules) for
     CPU, RAM, and time usage of the current session. -->

<script lang="ts">
  import { sandbox } from '$lib/stores/sandbox.svelte';

  let memPctRaw = $derived(
    sandbox.budget && sandbox.limits.memory_mib
      ? (sandbox.budget.memory_peak_bytes / 1024 / 1024) / sandbox.limits.memory_mib
      : 0
  );
  let memPct = $derived(Math.min(1, memPctRaw));
  let timePctRaw = $derived(
    sandbox.budget && sandbox.limits.time_secs
      ? (sandbox.budget.wall_time_ms_total / 1000) / sandbox.limits.time_secs
      : 0
  );
  let timePct = $derived(Math.min(1, timePctRaw));
  let cellsTotal = $derived(sandbox.budget?.cells_run ?? 0);
  let cellsOk = $derived(sandbox.budget?.cells_succeeded ?? 0);
  let cellsFail = $derived(sandbox.budget?.cells_failed ?? 0);
  let cellsLimit = $derived(sandbox.budget?.cells_limit_exceeded ?? 0);

  function gaugeColor(pct: number): string {
    if (pct < 0.6) return '#00FF66';
    if (pct < 0.85) return '#FF9933';
    return '#FF3333';
  }

  function describeArc(cx: number, cy: number, r: number, start: number, end: number): string {
    const startX = cx + r * Math.cos(start);
    const startY = cy + r * Math.sin(start);
    const endX = cx + r * Math.cos(end);
    const endY = cy + r * Math.sin(end);
    const largeArc = end - start > Math.PI ? 1 : 0;
    return `M ${startX} ${startY} A ${r} ${r} 0 ${largeArc} 1 ${endX} ${endY}`;
  }

  function arcFor(pct: number): string {
    const start = -Math.PI / 2;
    const end = start + 2 * Math.PI * pct;
    if (pct >= 1) {
      return describeArc(40, 40, 32, start, start + 2 * Math.PI - 0.001);
    }
    return describeArc(40, 40, 32, start, end);
  }
</script>

<div class="resource-monitor">
  <div class="gauge">
    <svg viewBox="0 0 80 80" width="80" height="80">
      <circle cx="40" cy="40" r="32" stroke="rgba(255,255,255,0.08)" stroke-width="6" fill="none" />
      <path d={arcFor(memPct)} stroke={gaugeColor(memPct)} stroke-width="6" fill="none" stroke-linecap="round" />
      <text x="40" y="44" text-anchor="middle" font-family="JetBrains Mono" font-size="13" fill="var(--gx-text-primary, #E8E8ED)">
        {Math.round(memPct * 100)}%
      </text>
    </svg>
    <div class="gauge-label">RAM</div>
  </div>

  <div class="gauge">
    <svg viewBox="0 0 80 80" width="80" height="80">
      <circle cx="40" cy="40" r="32" stroke="rgba(255,255,255,0.08)" stroke-width="6" fill="none" />
      <path d={arcFor(timePct)} stroke={gaugeColor(timePct)} stroke-width="6" fill="none" stroke-linecap="round" />
      <text x="40" y="44" text-anchor="middle" font-family="JetBrains Mono" font-size="13" fill="var(--gx-text-primary, #E8E8ED)">
        {Math.round(timePct * 100)}%
      </text>
    </svg>
    <div class="gauge-label">CPU time</div>
  </div>

  <div class="cell-stats">
    <div class="stat">
      <span class="stat-num">{cellsTotal}</span>
      <span class="stat-label">cells</span>
    </div>
    <div class="stat">
      <span class="stat-num text-success">{cellsOk}</span>
      <span class="stat-label">ok</span>
    </div>
    <div class="stat">
      <span class="stat-num text-error">{cellsFail}</span>
      <span class="stat-label">fail</span>
    </div>
    <div class="stat">
      <span class="stat-num text-warn">{cellsLimit}</span>
      <span class="stat-label">limit</span>
    </div>
  </div>
</div>

<style>
  .resource-monitor {
    display: flex;
    align-items: center;
    gap: 24px;
    padding: 12px 16px;
    background: var(--gx-bg-glass, rgba(19, 19, 26, 0.72));
    border: 1px solid var(--gx-border-default, #2A2A3A);
    border-radius: 12px;
    margin-bottom: 16px;
    backdrop-filter: blur(16px) saturate(120%);
  }
  .gauge {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }
  .gauge-label {
    font-family: 'JetBrains Mono', monospace;
    font-size: 10px;
    color: var(--gx-text-secondary, #A0A0B0);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .cell-stats {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 16px;
    margin-left: auto;
  }
  .stat {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
  }
  .stat-num {
    font-family: 'JetBrains Mono', monospace;
    font-size: 18px;
    font-weight: 600;
    color: var(--gx-text-primary, #E8E8ED);
  }
  .stat-num.text-success { color: var(--brand-neon, #00FF66); }
  .stat-num.text-error { color: var(--gx-error, #FF3333); }
  .stat-num.text-warn { color: var(--gx-warning, #FF9933); }
  .stat-label {
    font-family: 'JetBrains Mono', monospace;
    font-size: 10px;
    color: var(--gx-text-secondary, #A0A0B0);
    text-transform: uppercase;
  }
</style>
