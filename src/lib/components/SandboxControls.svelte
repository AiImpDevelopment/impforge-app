<!-- SPDX-License-Identifier: MIT -->
<!-- SandboxControls.svelte — global session controls (run-all, clear-all,
     export, import, pause/resume, language picker for new cell). -->

<script lang="ts">
  import { sandbox, type Lang } from '$lib/stores/sandbox.svelte';

  let { onAddCell }: { onAddCell?: (lang: Lang) => void } = $props();

  let newLang = $state<Lang>('python');
  let exporting = $state(false);
  let importing = $state(false);
  let paused = $state(false);

  async function exportNotebook() {
    exporting = true;
    try {
      const json = await sandbox.exportNotebook();
      const blob = new Blob([json], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `impforge-notebook-${sandbox.sessionId}.json`;
      a.click();
      URL.revokeObjectURL(url);
    } finally {
      exporting = false;
    }
  }

  function importNotebook() {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = 'application/json';
    input.onchange = async () => {
      if (!input.files?.length) return;
      importing = true;
      try {
        const text = await input.files[0].text();
        await sandbox.importNotebook(text);
      } finally {
        importing = false;
      }
    };
    input.click();
  }
</script>

<div class="sandbox-controls">
  <div class="left-group">
    <select bind:value={newLang} class="lang-picker">
      <option value="python">Python</option>
      <option value="javascript">JavaScript</option>
      <option value="wasm">Wasm</option>
    </select>
    <button type="button" onclick={() => onAddCell?.(newLang)} class="btn-primary">
      ＋ Add cell
    </button>
  </div>

  <div class="middle-group">
    <span class="session-id">session: {sandbox.sessionId.slice(0, 12)}…</span>
    {#if !sandbox.pyodideCached}
      <span class="pyodide-warn" title="Pyodide blob not cached — open settings to download">
        ⚠ Pyodide not cached
      </span>
    {/if}
  </div>

  <div class="right-group">
    <button
      type="button"
      class="btn-secondary"
      onclick={exportNotebook}
      disabled={exporting}
    >
      {exporting ? '…' : '⇩ Export'}
    </button>
    <button
      type="button"
      class="btn-secondary"
      onclick={importNotebook}
      disabled={importing}
    >
      {importing ? '…' : '⇧ Import'}
    </button>
    <button
      type="button"
      class="btn-secondary"
      onclick={() => (paused = !paused)}
      title={paused ? 'Resume session' : 'Pause session'}
    >
      {paused ? '▶ Resume' : '⏸ Pause'}
    </button>
  </div>
</div>

<style>
  .sandbox-controls {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 10px 16px;
    background: var(--gx-bg-glass, rgba(19, 19, 26, 0.72));
    border: 1px solid var(--gx-border-default, #2A2A3A);
    border-radius: 12px;
    margin-bottom: 16px;
    backdrop-filter: blur(16px) saturate(120%);
  }
  .left-group, .middle-group, .right-group {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .middle-group {
    flex: 1;
    justify-content: center;
  }
  .lang-picker {
    background: var(--gx-bg-secondary, #13131A);
    color: var(--gx-text-primary, #E8E8ED);
    border: 1px solid var(--gx-border-default, #2A2A3A);
    border-radius: 6px;
    padding: 6px 10px;
    font-family: 'Inter', sans-serif;
    font-size: 12px;
  }
  .btn-primary {
    background: var(--brand-neon, #00FF66);
    color: #000;
    border: none;
    border-radius: 6px;
    padding: 6px 12px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s ease;
  }
  .btn-primary:hover {
    transform: translateY(-1px);
    box-shadow: 0 0 16px rgba(0, 255, 102, 0.3);
  }
  .btn-secondary {
    background: transparent;
    color: var(--gx-text-secondary, #A0A0B0);
    border: 1px solid var(--gx-border-default, #2A2A3A);
    border-radius: 6px;
    padding: 6px 12px;
    cursor: pointer;
    transition: all 0.15s ease;
  }
  .btn-secondary:hover {
    border-color: var(--brand-neon, #00FF66);
    color: var(--brand-neon, #00FF66);
  }
  .session-id {
    font-family: 'JetBrains Mono', monospace;
    font-size: 11px;
    color: var(--gx-text-secondary, #A0A0B0);
  }
  .pyodide-warn {
    color: var(--gx-warning, #FF9933);
    font-size: 11px;
    padding: 2px 8px;
    border: 1px solid var(--gx-warning, #FF9933);
    border-radius: 4px;
  }
</style>
