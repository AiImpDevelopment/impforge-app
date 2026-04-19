<!-- SPDX-License-Identifier: MIT -->
<!--
  Inline pill rendered inside chat messages whenever the agent calls a
  tool.  Click to expand into args + result + latency breakdown.

  Format inspired by the research §C.1 "X-ray for AI" — nobody else
  ships this.

  Pro adds: full Merkle proof for the call (via ToolCallXrayPro.svelte).
-->
<script lang="ts">
  interface Props {
    serverId: string;
    toolName: string;
    args: Record<string, unknown>;
    result: unknown;
    latencyMs: number;
    success: boolean;
    /** Optional click-handler the parent provides to surface upgrade nudge. */
    onProUpgradeClick?: () => void;
  }

  let { serverId, toolName, args, result, latencyMs, success, onProUpgradeClick }: Props = $props();

  let expanded = $state(false);

  const argsPretty = $derived.by(() => {
    try {
      return JSON.stringify(args, null, 2);
    } catch {
      return String(args);
    }
  });
  const resultPretty = $derived.by(() => {
    try {
      return JSON.stringify(result, null, 2);
    } catch {
      return String(result);
    }
  });

  /** Detect zero-width chars in args (Invariant Labs tool-poisoning notification). */
  const hasZeroWidth = $derived.by(() => {
    const json = JSON.stringify(args);
    return /[\u200B\u200C\u200D\uFEFF]/.test(json);
  });
</script>

<span class="pill" class:success class:failure={!success} class:tampered={hasZeroWidth}>
  <button type="button" class="pill-button" onclick={() => (expanded = !expanded)} aria-expanded={expanded}>
    <span class="dot" aria-hidden="true"></span>
    <code class="server mono">{serverId}</code>
    <span class="dot-separator">·</span>
    <code class="tool mono">{toolName}</code>
    <span class="latency mono">{latencyMs}ms</span>
    {#if hasZeroWidth}
      <span class="warn-icon" title="Hidden Unicode chars detected in arguments — possible injection attempt">⚠</span>
    {/if}
    <span class="expand-icon" aria-hidden="true">{expanded ? '−' : '+'}</span>
  </button>
</span>

{#if expanded}
  <div class="drawer">
    <div class="block">
      <h4>Arguments</h4>
      <pre class="json mono">{argsPretty}</pre>
      {#if hasZeroWidth}
        <p class="warn">⚠ Zero-width Unicode characters detected.  Hover the args text — these may be invisible injection payloads.</p>
      {/if}
    </div>
    <div class="block">
      <h4>Result</h4>
      <pre class="json mono">{resultPretty}</pre>
    </div>
    <div class="block">
      <h4>Telemetry</h4>
      <ul class="telemetry mono">
        <li><span>server</span> <span>{serverId}</span></li>
        <li><span>tool</span> <span>{toolName}</span></li>
        <li><span>latency</span> <span>{latencyMs}ms</span></li>
        <li><span>status</span> <span class:success-text={success} class:failure-text={!success}>{success ? 'success' : 'failure'}</span></li>
      </ul>
    </div>
    <button
      type="button"
      class="upgrade"
      onclick={() => onProUpgradeClick?.()}
      aria-label="Learn how Pro audits this call"
    >
      🔒 In ImpForge Pro: full Merkle proof + per-call attestation →
    </button>
  </div>
{/if}

<style>
  .pill {
    display: inline-flex;
    margin: 0 4px;
  }
  .pill-button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: rgba(19, 19, 26, 0.85);
    border: 1px solid #2A2A3A;
    color: #E8E8ED;
    padding: 4px 10px;
    border-radius: 999px;
    font-size: 11px;
    cursor: pointer;
    transition: all 0.2s;
  }
  .pill-button:hover {
    border-color: #00CCFF;
    box-shadow: 0 0 8px rgba(0, 204, 255, 0.3);
  }
  .pill.success .dot {
    background: #00FF66;
    box-shadow: 0 0 4px #00FF66;
  }
  .pill.failure .dot {
    background: #FF3366;
    box-shadow: 0 0 4px #FF3366;
  }
  .pill.tampered .pill-button {
    border-color: #FF8800;
    box-shadow: 0 0 8px rgba(255, 136, 0, 0.3);
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #A0A0B0;
  }
  .server {
    color: #00CCFF;
    font-size: 11px;
  }
  .tool {
    color: #E8E8ED;
    font-size: 11px;
  }
  .latency {
    color: #606070;
    font-size: 10px;
  }
  .dot-separator {
    color: #606070;
  }
  .warn-icon {
    color: #FF8800;
  }
  .expand-icon {
    color: #606070;
    margin-left: 2px;
    font-family: 'JetBrains Mono', monospace;
  }
  .drawer {
    margin: 8px 0;
    background: rgba(8, 8, 13, 0.85);
    border: 1px solid #2A2A3A;
    border-radius: 10px;
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    backdrop-filter: blur(8px) saturate(120%);
    animation: drawerOpen 0.2s ease;
  }
  .block {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .block h4 {
    font-family: 'Space Grotesk', sans-serif;
    font-weight: 600;
    font-size: 11px;
    color: #00CCFF;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin: 0;
  }
  .json {
    background: rgba(13, 13, 18, 0.7);
    border: 1px solid #1F1F2A;
    border-radius: 6px;
    padding: 8px 10px;
    font-size: 11px;
    color: #E8E8ED;
    overflow-x: auto;
    margin: 0;
    line-height: 1.5;
    max-height: 200px;
    overflow-y: auto;
  }
  .warn {
    color: #FF8800;
    font-size: 11px;
    margin: 0;
    font-family: 'Inter', sans-serif;
  }
  .telemetry {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .telemetry li {
    display: flex;
    justify-content: space-between;
    color: #A0A0B0;
    font-size: 11px;
  }
  .telemetry li span:first-child {
    color: #606070;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .success-text {
    color: #00FF66;
  }
  .failure-text {
    color: #FF3366;
  }
  .upgrade {
    background: linear-gradient(135deg, rgba(153, 102, 255, 0.12), rgba(255, 51, 153, 0.08));
    border: 1px solid rgba(153, 102, 255, 0.4);
    color: #9966FF;
    padding: 8px 12px;
    border-radius: 8px;
    cursor: pointer;
    font-family: 'Space Grotesk', sans-serif;
    font-size: 12px;
    font-weight: 500;
    transition: all 0.2s;
    text-align: left;
  }
  .upgrade:hover {
    border-color: #9966FF;
    box-shadow: 0 0 12px rgba(153, 102, 255, 0.25);
  }
  .mono {
    font-family: 'JetBrains Mono', ui-monospace, monospace;
  }
  @keyframes drawerOpen {
    from { opacity: 0; transform: translateY(-4px); }
    to { opacity: 1; transform: translateY(0); }
  }
  @media (prefers-reduced-motion: reduce) {
    .drawer { animation: none; }
    .pill-button,
    .upgrade { transition: none; }
  }
</style>
