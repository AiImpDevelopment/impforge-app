<!-- SPDX-License-Identifier: MIT -->
<!-- CellOutput.svelte — render one Output entry (text/image/html/table/error/result/file). -->

<script lang="ts">
  import type { Output } from '$lib/stores/sandbox.svelte';

  let { output }: { output: Output } = $props();

  function streamColor(stream: 'stdout' | 'stderr'): string {
    return stream === 'stdout' ? 'text-gx-text-primary' : 'text-gx-warning';
  }

  function sizeStr(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KiB`;
    return `${(n / (1024 * 1024)).toFixed(1)} MiB`;
  }
</script>

<div class="cell-output">
  {#if output.kind === 'text'}
    <pre class="text-output {streamColor(output.stream)}" data-stream={output.stream}>{output.data}</pre>
  {:else if output.kind === 'image'}
    <div class="image-output">
      <img src={`data:${output.mime};base64,${output.base64}`} alt="cell image output" />
    </div>
  {:else if output.kind === 'html'}
    <div class="html-output">{@html output.data}</div>
  {:else if output.kind === 'table'}
    <table class="table-output">
      <thead>
        <tr>
          {#each output.headers as h (h)}
            <th>{h}</th>
          {/each}
        </tr>
      </thead>
      <tbody>
        {#each output.rows as row (row)}
          <tr>
            {#each row as cell (cell)}
              <td>{cell}</td>
            {/each}
          </tr>
        {/each}
      </tbody>
    </table>
  {:else if output.kind === 'file'}
    <div class="file-output">
      <span class="file-icon">📄</span>
      <span class="file-path">{output.path}</span>
      <span class="file-mime">{output.mime}</span>
      <span class="file-size">{sizeStr(output.bytes)}</span>
    </div>
  {:else if output.kind === 'error'}
    <div class="error-output">
      <div class="error-name">{output.ename}: {output.evalue}</div>
      {#if output.traceback?.length}
        <pre class="error-traceback">{output.traceback.join('\n')}</pre>
      {/if}
    </div>
  {:else if output.kind === 'result'}
    <pre class="result-output">{output.repr}</pre>
  {/if}
</div>

<style>
  .cell-output {
    font-family: 'JetBrains Mono', monospace;
    font-size: 12px;
    line-height: 1.5;
  }
  .text-output {
    margin: 0;
    padding: 4px 0;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .text-output[data-stream='stderr'] {
    color: var(--gx-warning, #FF9933);
  }
  .image-output img {
    max-width: 100%;
    border-radius: 6px;
    border: 1px solid var(--gx-border-default, #2A2A3A);
  }
  .html-output {
    color: var(--gx-text-primary, #E8E8ED);
  }
  .table-output {
    border-collapse: collapse;
    width: 100%;
  }
  .table-output th, .table-output td {
    border: 1px solid var(--gx-border-default, #2A2A3A);
    padding: 4px 8px;
    text-align: left;
  }
  .table-output th {
    background: var(--gx-bg-glass, rgba(19,19,26,0.72));
  }
  .file-output {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    background: var(--gx-bg-glass, rgba(19,19,26,0.72));
    border-radius: 6px;
  }
  .file-icon {
    font-size: 16px;
  }
  .file-path {
    flex: 1;
    color: var(--gx-text-primary, #E8E8ED);
  }
  .file-mime, .file-size {
    color: var(--gx-text-secondary, #A0A0B0);
    font-size: 10px;
  }
  .error-output {
    background: rgba(255, 51, 51, 0.08);
    border-left: 3px solid var(--gx-error, #FF3333);
    padding: 8px 12px;
    border-radius: 4px;
  }
  .error-name {
    color: var(--gx-error, #FF3333);
    font-weight: 600;
  }
  .error-traceback {
    margin: 6px 0 0 0;
    font-size: 11px;
    color: var(--gx-text-secondary, #A0A0B0);
    white-space: pre-wrap;
  }
  .result-output {
    background: rgba(0, 255, 102, 0.05);
    border-left: 3px solid var(--brand-neon, #00FF66);
    padding: 6px 12px;
    margin: 0;
    border-radius: 4px;
    color: var(--brand-neon, #00FF66);
  }
</style>
