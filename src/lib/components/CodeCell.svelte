<!-- SPDX-License-Identifier: MIT -->
<!-- CodeCell.svelte — Jupyter-style cell with run controls + outputs.
     Wires to code_sandbox_lite/mod.rs Tauri commands via the sandbox store.
     Cybercrime liability defence: see src-tauri/code_sandbox_lite/mod.rs. -->

<script lang="ts">
  import { sandbox, type Lang, type TrustLevel, type Cell, type CellResult } from '$lib/stores/sandbox.svelte';
  import CellOutput from './CellOutput.svelte';

  let {
    cell = null as Cell | null,
    initialSource = '',
    initialLang = 'python' as Lang,
  }: {
    cell?: Cell | null;
    initialSource?: string;
    initialLang?: Lang;
  } = $props();

  let source = $state(cell?.source ?? initialSource);
  let lang = $state<Lang>(cell?.lang ?? initialLang);
  let trustLevel = $state<TrustLevel>(cell?.trust_level ?? 'low');
  let preopenPath = $state('');
  let running = $state(false);
  let error = $state<string | null>(null);
  let result = $derived<CellResult | null>(cell ? sandbox.results.get(cell.id) ?? null : null);
  let collapsed = $state(false);
  let showSettings = $state(false);

  async function runCell() {
    if (running) return;
    error = null;
    running = true;
    try {
      let target = cell;
      if (!target) {
        target = await sandbox.createCell({
          lang,
          source,
          trustLevel,
          preopen: preopenPath || undefined,
        });
      }
      await sandbox.runCell(target.id);
    } catch (e) {
      error = String(e);
    } finally {
      running = false;
    }
  }

  async function stopCell() {
    if (!cell) return;
    await sandbox.stopCell(cell.id);
  }

  async function clearOutputs() {
    if (!cell) return;
    await sandbox.clearCell(cell.id);
  }

  function statusColor(): string {
    if (!result) return 'text-gx-text-secondary';
    switch (result.status) {
      case 'succeeded': return 'text-gx-success';
      case 'failed': return 'text-gx-error';
      case 'running': return 'text-gx-warning';
      case 'limit_exceeded': return 'text-gx-warning';
      case 'cancelled': return 'text-gx-text-secondary';
      default: return 'text-gx-text-secondary';
    }
  }
</script>

<div class="code-cell">
  <div class="cell-header">
    <button
      type="button"
      class="collapse-btn"
      onclick={() => (collapsed = !collapsed)}
      aria-label={collapsed ? 'Expand cell' : 'Collapse cell'}
    >
      {collapsed ? '▶' : '▼'}
    </button>
    <select bind:value={lang} class="lang-picker" disabled={running}>
      <option value="python">Python (Pyodide)</option>
      <option value="javascript">JavaScript (rquickjs)</option>
      <option value="wasm">WebAssembly (raw)</option>
    </select>
    <span class="status {statusColor()}">
      {result?.status ?? 'pending'}
    </span>
    <span class="usage" title="fuel · time · memory">
      {result ? `${result.usage.fuel_consumed} fuel · ${result.usage.wall_time_ms} ms` : '—'}
    </span>
    <div class="cell-actions">
      <button type="button" onclick={runCell} disabled={running} class="btn-run">
        {running ? '…' : '▶ Run'}
      </button>
      {#if running}
        <button type="button" onclick={stopCell} class="btn-stop">■ Stop</button>
      {/if}
      <button type="button" onclick={clearOutputs} class="btn-clear">Clear</button>
      <button
        type="button"
        onclick={() => (showSettings = !showSettings)}
        class="btn-settings"
        aria-label="Cell settings"
      >
        ⚙
      </button>
    </div>
  </div>

  {#if showSettings}
    <div class="cell-settings">
      <label>
        Trust level:
        <select bind:value={trustLevel}>
          <option value="low">Low (no fs, no net)</option>
          <option value="medium">Medium (read-only mount)</option>
          <option value="high">High (Pro only)</option>
        </select>
      </label>
      <label>
        Preopen path:
        <input
          type="text"
          bind:value={preopenPath}
          placeholder="/home/user/data (optional)"
        />
      </label>
    </div>
  {/if}

  {#if !collapsed}
    <textarea
      bind:value={source}
      class="cell-source"
      placeholder={lang === 'python' ? 'print(1+1)' : lang === 'javascript' ? 'console.log(1+1)' : '(module ...)'}
      spellcheck="false"
      disabled={running}
    ></textarea>

    {#if error}
      <div class="cell-error">
        {error}
      </div>
    {/if}

    {#if result}
      <div class="cell-outputs">
        {#each result.outputs as out (out)}
          <CellOutput output={out} />
        {/each}
      </div>
    {/if}
  {/if}
</div>

<style>
  .code-cell {
    background: var(--gx-bg-secondary, #13131A);
    border: 1px solid var(--gx-border-default, #2A2A3A);
    border-radius: 12px;
    margin-bottom: 16px;
    overflow: hidden;
  }
  .cell-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    background: var(--gx-bg-glass, rgba(19, 19, 26, 0.72));
    border-bottom: 1px solid var(--gx-border-default, #2A2A3A);
  }
  .collapse-btn {
    background: transparent;
    border: none;
    color: var(--gx-text-secondary, #A0A0B0);
    cursor: pointer;
    font-size: 12px;
    padding: 0 4px;
  }
  .lang-picker {
    background: var(--gx-bg-secondary, #13131A);
    color: var(--gx-text-primary, #E8E8ED);
    border: 1px solid var(--gx-border-default, #2A2A3A);
    border-radius: 6px;
    padding: 4px 8px;
    font-family: 'Inter', sans-serif;
    font-size: 12px;
  }
  .status {
    font-family: 'JetBrains Mono', monospace;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 2px 8px;
    border-radius: 4px;
  }
  .usage {
    font-family: 'JetBrains Mono', monospace;
    font-size: 11px;
    color: var(--gx-text-secondary, #A0A0B0);
  }
  .cell-actions {
    margin-left: auto;
    display: flex;
    gap: 6px;
  }
  .btn-run {
    background: var(--brand-neon, #00FF66);
    color: #000;
    border: none;
    border-radius: 6px;
    padding: 4px 12px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s ease;
  }
  .btn-run:hover {
    transform: translateY(-1px);
    box-shadow: 0 0 12px rgba(0, 255, 102, 0.4);
  }
  .btn-run:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .btn-stop {
    background: var(--accent-orange, #FF6633);
    color: #000;
    border: none;
    border-radius: 6px;
    padding: 4px 12px;
    font-size: 12px;
    cursor: pointer;
  }
  .btn-clear, .btn-settings {
    background: transparent;
    color: var(--gx-text-secondary, #A0A0B0);
    border: 1px solid var(--gx-border-default, #2A2A3A);
    border-radius: 6px;
    padding: 4px 8px;
    font-size: 11px;
    cursor: pointer;
  }
  .cell-settings {
    padding: 8px 12px;
    border-bottom: 1px solid var(--gx-border-default, #2A2A3A);
    display: grid;
    gap: 6px;
    font-size: 12px;
    color: var(--gx-text-secondary, #A0A0B0);
  }
  .cell-settings input,
  .cell-settings select {
    background: var(--gx-bg-secondary, #13131A);
    color: var(--gx-text-primary, #E8E8ED);
    border: 1px solid var(--gx-border-default, #2A2A3A);
    border-radius: 4px;
    padding: 4px 8px;
    font-family: 'JetBrains Mono', monospace;
  }
  .cell-source {
    width: 100%;
    min-height: 100px;
    background: var(--gx-bg-primary, #0D0D12);
    color: var(--gx-text-primary, #E8E8ED);
    border: none;
    padding: 12px;
    font-family: 'JetBrains Mono', monospace;
    font-size: 13px;
    line-height: 1.5;
    resize: vertical;
    box-sizing: border-box;
  }
  .cell-source:focus {
    outline: none;
    background: var(--gx-bg-primary, #0D0D12);
  }
  .cell-error {
    background: rgba(255, 51, 51, 0.1);
    color: var(--gx-error, #FF3333);
    padding: 8px 12px;
    font-family: 'JetBrains Mono', monospace;
    font-size: 12px;
    border-top: 1px solid var(--gx-error, #FF3333);
  }
  .cell-outputs {
    border-top: 1px solid var(--gx-border-default, #2A2A3A);
    padding: 8px 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
</style>
