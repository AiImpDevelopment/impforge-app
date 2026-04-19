<!-- SPDX-License-Identifier: MIT -->
<!-- Sandbox notebook page — assembles CodeCell + CellOutput +
     SandboxControls + ResourceMonitor into a Jupyter-style page. -->

<script lang="ts">
  import { onMount } from 'svelte';
  import { sandbox, type Lang, type Cell } from '$lib/stores/sandbox.svelte';
  import CodeCell from '$lib/components/CodeCell.svelte';
  import SandboxControls from '$lib/components/SandboxControls.svelte';
  import ResourceMonitor from '$lib/components/ResourceMonitor.svelte';

  let cells = $state<Cell[]>([]);
  let initSource = $state<Record<Lang, string>>({
    python: 'print(1+1)',
    javascript: 'console.log(1+1)',
    wasm: '(module\n  (import "wasi_snapshot_preview1" "proc_exit" (func $e (param i32)))\n  (memory (export "memory") 1)\n  (func $start i32.const 0 call $e)\n  (export "_start" (func $start)))',
  });

  onMount(async () => {
    await sandbox.healthCheck();
    await sandbox.refreshBudget();
  });

  async function addCell(lang: Lang) {
    const cell = await sandbox.createCell({
      lang,
      source: initSource[lang],
      trustLevel: 'low',
    });
    cells = [...cells, cell];
  }

  async function downloadPyodide() {
    // Currently a no-op stub — Tier 2 surface ships next chunk.
    alert('Pyodide download UI: implemented in next PR.\nFor now, use `impforge-cli download-pyodide`.');
  }
</script>

<div class="sandbox-page">
  <header class="sandbox-header">
    <h1>Code Interpreter Sandbox</h1>
    <p class="subtitle">
      Jupyter-style notebook · wasmtime + Pyodide + rquickjs ·
      <span class="badge-mit">MIT freemium</span> ·
      <a href="https://impforge.com/pro">upgrade for Firecracker microVMs + GPU passthrough + Cortex Veto</a>
    </p>
  </header>

  <ResourceMonitor />
  <SandboxControls onAddCell={addCell} />

  {#if cells.length === 0}
    <div class="empty-state">
      <p>No cells yet.  Pick a language and click "+ Add cell" to start.</p>
      <p class="empty-hint">
        Every cell runs in a wasmtime sandbox on YOUR machine.  No code, no
        outputs and no variables ever leave your computer.
      </p>
      {#if !sandbox.pyodideCached}
        <button type="button" class="btn-download" onclick={downloadPyodide}>
          Download Pyodide (~30 MiB, one-time)
        </button>
      {/if}
    </div>
  {:else}
    <div class="cells">
      {#each cells as cell (cell.id)}
        <CodeCell {cell} />
      {/each}
    </div>
  {/if}

  {#if sandbox.lastError}
    <div class="last-error" role="alert">
      {sandbox.lastError}
    </div>
  {/if}
</div>

<style>
  .sandbox-page {
    max-width: 1024px;
    margin: 0 auto;
    padding: 24px;
    background: linear-gradient(135deg, #0D0D12 0%, #13131A 50%, #0D1117 100%);
    min-height: 100vh;
  }
  .sandbox-header h1 {
    font-family: 'Space Grotesk', sans-serif;
    font-size: 28px;
    color: var(--gx-text-primary, #E8E8ED);
    margin: 0 0 4px 0;
    letter-spacing: -0.02em;
  }
  .subtitle {
    color: var(--gx-text-secondary, #A0A0B0);
    font-size: 13px;
    margin-bottom: 16px;
  }
  .badge-mit {
    background: var(--brand-neon, #00FF66);
    color: #000;
    padding: 1px 8px;
    border-radius: 4px;
    font-size: 11px;
    font-weight: 600;
  }
  .empty-state {
    text-align: center;
    padding: 48px 24px;
    color: var(--gx-text-secondary, #A0A0B0);
    border: 1px dashed var(--gx-border-default, #2A2A3A);
    border-radius: 12px;
  }
  .empty-hint {
    font-size: 12px;
    color: var(--gx-text-secondary, #A0A0B0);
    opacity: 0.7;
    margin: 8px 0 16px 0;
  }
  .btn-download {
    background: var(--brand-neon, #00FF66);
    color: #000;
    border: none;
    border-radius: 6px;
    padding: 8px 16px;
    font-weight: 600;
    cursor: pointer;
    margin-top: 12px;
    transition: all 0.15s ease;
  }
  .btn-download:hover {
    transform: translateY(-1px);
    box-shadow: 0 0 16px rgba(0, 255, 102, 0.4);
  }
  .cells {
    display: flex;
    flex-direction: column;
  }
  .last-error {
    margin-top: 16px;
    padding: 12px 16px;
    background: rgba(255, 51, 51, 0.08);
    border: 1px solid var(--gx-error, #FF3333);
    color: var(--gx-error, #FF3333);
    border-radius: 8px;
    font-family: 'JetBrains Mono', monospace;
    font-size: 12px;
  }
</style>
