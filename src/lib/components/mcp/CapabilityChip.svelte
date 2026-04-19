<!-- SPDX-License-Identifier: MIT -->
<script lang="ts">
  interface Props {
    icon?: 'tools' | 'resources' | 'prompts' | 'oauth' | 'local' | 'remote';
    label: string;
    accent?: 'cyan' | 'magenta' | 'orange' | 'purple' | 'neon';
  }
  let { icon, label, accent = 'cyan' }: Props = $props();

  const colour = $derived.by(() => {
    switch (accent) {
      case 'cyan': return '#00CCFF';
      case 'magenta': return '#FF3399';
      case 'orange': return '#FF8800';
      case 'purple': return '#9966FF';
      case 'neon': return '#00FF66';
      default: return '#A0A0B0';
    }
  });
</script>

<span class="chip" style="--accent: {colour}">
  {#if icon}
    <span class="dot" aria-hidden="true"></span>
  {/if}
  <span class="label">{label}</span>
</span>

<style>
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 10px;
    border-radius: 999px;
    background: rgba(19, 19, 26, 0.72);
    border: 1px solid color-mix(in srgb, var(--accent) 30%, transparent);
    color: var(--accent);
    font-family: 'JetBrains Mono', ui-monospace, monospace;
    font-size: 11px;
    line-height: 1.4;
    transition: all 0.2s ease;
    white-space: nowrap;
  }
  .chip:hover {
    background: color-mix(in srgb, var(--accent) 12%, rgba(19, 19, 26, 0.85));
    border-color: var(--accent);
    box-shadow: 0 0 8px color-mix(in srgb, var(--accent) 40%, transparent);
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 4px var(--accent);
  }
  .label {
    font-weight: 500;
  }
  @media (prefers-reduced-motion: reduce) {
    .chip {
      transition: none;
    }
  }
</style>
