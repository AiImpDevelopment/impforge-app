<!-- SPDX-License-Identifier: MIT -->
<!--
  Coloured diff before update — rug-pull defence per ETDI arXiv 2506.01333.
  Added items in neon-green, removed in muted, changed in cyan, escalations
  in orange.  User must click "Approve" before `mcp_update_server` is called.
-->
<script lang="ts">
  interface ToolDiff {
    name: string;
    changes: string[];
  }

  interface CapabilityDiff {
    server_id: string;
    from_version: string;
    to_version: string;
    added_tools: { name: string; description: string }[];
    removed_tools: { name: string; description: string }[];
    changed_tools: ToolDiff[];
    added_egress_hosts: string[];
    removed_egress_hosts: string[];
    added_fs_paths: string[];
    removed_fs_paths: string[];
    privilege_escalation: boolean;
  }

  interface Props {
    diff: CapabilityDiff;
    onApprove?: () => void;
    onCancel?: () => void;
  }

  let { diff, onApprove, onCancel }: Props = $props();

  const totalChanges = $derived(
    diff.added_tools.length +
      diff.removed_tools.length +
      diff.changed_tools.length +
      diff.added_egress_hosts.length +
      diff.removed_egress_hosts.length +
      diff.added_fs_paths.length +
      diff.removed_fs_paths.length,
  );
</script>

<section class="diff" class:escalation={diff.privilege_escalation}>
  <header>
    <div class="version-row">
      <span class="server-id">{diff.server_id}</span>
      <span class="arrow">→</span>
      <span class="version mono">{diff.from_version}</span>
      <span class="arrow">→</span>
      <span class="version mono next">{diff.to_version}</span>
    </div>
    {#if diff.privilege_escalation}
      <div class="escalation-banner">
        <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
          <path
            d="M8 1l7 14H1z M8 6v4 M8 12v0.5"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linejoin="round"
          />
        </svg>
        <span>Privilege escalation detected — server gains new capabilities. Re-consent required.</span>
      </div>
    {/if}
    <p class="summary">
      {totalChanges} change{totalChanges === 1 ? '' : 's'}
    </p>
  </header>

  {#if diff.added_tools.length > 0}
    <section class="block added">
      <h4>+ Added tools ({diff.added_tools.length})</h4>
      <ul>
        {#each diff.added_tools as tool (tool.name)}
          <li>
            <span class="tool-name mono">{tool.name}</span>
            <span class="tool-desc">{tool.description}</span>
          </li>
        {/each}
      </ul>
    </section>
  {/if}

  {#if diff.removed_tools.length > 0}
    <section class="block removed">
      <h4>− Removed tools ({diff.removed_tools.length})</h4>
      <ul>
        {#each diff.removed_tools as tool (tool.name)}
          <li>
            <span class="tool-name mono">{tool.name}</span>
            <span class="tool-desc">{tool.description}</span>
          </li>
        {/each}
      </ul>
    </section>
  {/if}

  {#if diff.changed_tools.length > 0}
    <section class="block changed">
      <h4>~ Changed tools ({diff.changed_tools.length})</h4>
      <ul>
        {#each diff.changed_tools as tool (tool.name)}
          <li>
            <div class="tool-name mono">{tool.name}</div>
            <ul class="changes">
              {#each tool.changes as change}
                <li class:warning={change.includes('added')}>{change}</li>
              {/each}
            </ul>
          </li>
        {/each}
      </ul>
    </section>
  {/if}

  {#if diff.added_egress_hosts.length > 0 || diff.removed_egress_hosts.length > 0}
    <section class="block">
      <h4>Egress hosts</h4>
      {#each diff.added_egress_hosts as host}
        <div class="path-line added-line">+ {host}</div>
      {/each}
      {#each diff.removed_egress_hosts as host}
        <div class="path-line removed-line">− {host}</div>
      {/each}
    </section>
  {/if}

  {#if diff.added_fs_paths.length > 0 || diff.removed_fs_paths.length > 0}
    <section class="block">
      <h4>Filesystem paths</h4>
      {#each diff.added_fs_paths as p}
        <div class="path-line added-line">+ {p}</div>
      {/each}
      {#each diff.removed_fs_paths as p}
        <div class="path-line removed-line">− {p}</div>
      {/each}
    </section>
  {/if}

  <footer>
    <button type="button" class="btn ghost" onclick={() => onCancel?.()}>Cancel</button>
    <button
      type="button"
      class="btn primary"
      class:warning={diff.privilege_escalation}
      onclick={() => onApprove?.()}
    >
      {diff.privilege_escalation ? 'I understand · Approve update' : 'Approve update'}
    </button>
  </footer>
</section>

<style>
  .diff {
    background: linear-gradient(135deg, rgba(19, 19, 26, 0.92) 0%, rgba(13, 13, 18, 0.95) 100%);
    border: 1px solid #2A2A3A;
    border-radius: 14px;
    padding: 22px;
    display: flex;
    flex-direction: column;
    gap: 18px;
    backdrop-filter: blur(16px) saturate(120%);
  }
  .diff.escalation {
    border-color: rgba(255, 136, 0, 0.4);
    box-shadow: 0 0 24px rgba(255, 136, 0, 0.12);
  }

  header {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .version-row {
    display: flex;
    align-items: center;
    gap: 10px;
    font-family: 'Space Grotesk', system-ui, sans-serif;
    color: #E8E8ED;
  }
  .server-id {
    font-size: 18px;
    font-weight: 600;
  }
  .arrow {
    color: #606070;
  }
  .version {
    font-size: 14px;
    color: #A0A0B0;
  }
  .version.next {
    color: #00FF66;
  }
  .escalation-banner {
    display: flex;
    align-items: center;
    gap: 10px;
    background: rgba(255, 136, 0, 0.1);
    border: 1px solid rgba(255, 136, 0, 0.4);
    color: #FF8800;
    padding: 10px 14px;
    border-radius: 10px;
    font-family: 'Inter', sans-serif;
    font-size: 13px;
  }
  .summary {
    color: #606070;
    font-family: 'JetBrains Mono', ui-monospace, monospace;
    font-size: 12px;
    margin: 0;
  }

  .block {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px;
    background: rgba(19, 19, 26, 0.6);
    border-radius: 10px;
    border: 1px solid #1F1F2A;
  }
  .block h4 {
    margin: 0 0 6px 0;
    font-family: 'Space Grotesk', system-ui, sans-serif;
    font-size: 13px;
    color: #E8E8ED;
  }
  .block.added h4 {
    color: #00FF66;
  }
  .block.removed h4 {
    color: #A0A0B0;
  }
  .block.changed h4 {
    color: #00CCFF;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  li {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .tool-name {
    color: #E8E8ED;
    font-size: 13px;
    font-weight: 500;
  }
  .tool-desc {
    color: #A0A0B0;
    font-family: 'Inter', sans-serif;
    font-size: 12px;
  }
  .changes {
    margin: 4px 0 0 12px;
    font-family: 'JetBrains Mono', monospace;
    font-size: 11px;
    color: #A0A0B0;
  }
  .changes li.warning {
    color: #FF8800;
  }

  .path-line {
    font-family: 'JetBrains Mono', monospace;
    font-size: 12px;
    padding: 2px 6px;
    border-radius: 4px;
  }
  .added-line {
    color: #00FF66;
    background: rgba(0, 255, 102, 0.06);
  }
  .removed-line {
    color: #606070;
    background: rgba(96, 96, 112, 0.06);
  }

  footer {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    padding-top: 8px;
    border-top: 1px solid #1F1F2A;
  }
  .btn {
    padding: 9px 18px;
    border-radius: 8px;
    font-family: 'Space Grotesk', sans-serif;
    font-weight: 600;
    font-size: 13px;
    cursor: pointer;
    border: 1px solid transparent;
    transition: all 0.2s ease;
  }
  .btn.ghost {
    background: transparent;
    color: #A0A0B0;
    border-color: #2A2A3A;
  }
  .btn.ghost:hover {
    color: #E8E8ED;
    border-color: #A0A0B0;
  }
  .btn.primary {
    background: linear-gradient(135deg, #00FF66 0%, #00CC52 100%);
    color: #0D0D12;
  }
  .btn.primary:hover {
    box-shadow: 0 0 16px rgba(0, 255, 102, 0.4);
  }
  .btn.primary.warning {
    background: linear-gradient(135deg, #FF8800 0%, #CC6600 100%);
  }
  .btn.primary.warning:hover {
    box-shadow: 0 0 16px rgba(255, 136, 0, 0.4);
  }
  @media (prefers-reduced-motion: reduce) {
    .btn {
      transition: none;
    }
  }
</style>
