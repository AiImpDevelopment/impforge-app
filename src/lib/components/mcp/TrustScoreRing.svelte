<!-- SPDX-License-Identifier: MIT -->
<!--
  Circular gauge for the trust score (0-100).  ImpForge design rule:
  ALWAYS use circular gauges for system stats — NEVER plain progress
  bars.  Stroke colour shifts neon-green / cyan / orange / dim per
  bucket.  Hover surfaces the per-component breakdown via tooltip.
-->
<script lang="ts">
  import type { TrustScore } from '$lib/stores/mcp/marketplace.svelte';

  interface Props {
    score: TrustScore | null;
    size?: number;
    showLabel?: boolean;
  }

  let { score = null, size = 56, showLabel = true }: Props = $props();

  const radius = $derived(size / 2 - 4);
  const circumference = $derived(2 * Math.PI * radius);
  const value = $derived(score?.score ?? 0);
  const offset = $derived(circumference - (value / 100) * circumference);

  const colour = $derived.by(() => {
    if (value >= 80) return '#00FF66'; // neon
    if (value >= 50) return '#00CCFF'; // cyan
    if (value >= 30) return '#FF8800'; // orange
    return '#A0A0B0'; // muted
  });

  const tooltip = $derived.by(() => {
    if (!score) return 'computing trust score…';
    return [
      `verified: +${score.verified_pts}`,
      `signed: +${score.signed_pts}`,
      `installs: +${score.installs_pts}`,
      `stars: +${score.stars_pts}`,
      `recency: +${score.recency_pts}`,
      `sandbox: +${score.sandbox_pts}`,
    ].join('\n');
  });
</script>

<div class="ring" style="width: {size}px; height: {size}px;" title={tooltip} role="img" aria-label="trust score {value} of 100">
  <svg width={size} height={size} viewBox="0 0 {size} {size}">
    <!-- Track -->
    <circle
      cx={size / 2}
      cy={size / 2}
      r={radius}
      stroke="rgba(42, 42, 58, 0.6)"
      stroke-width="3"
      fill="none"
    />
    <!-- Progress -->
    <circle
      cx={size / 2}
      cy={size / 2}
      r={radius}
      stroke={colour}
      stroke-width="3"
      fill="none"
      stroke-linecap="round"
      stroke-dasharray={circumference}
      stroke-dashoffset={offset}
      transform="rotate(-90 {size / 2} {size / 2})"
      class="progress"
    />
    {#if showLabel}
      <text
        x="50%"
        y="50%"
        text-anchor="middle"
        dominant-baseline="central"
        fill={colour}
        font-family="JetBrains Mono, monospace"
        font-size={size / 3.2}
        font-weight="600"
      >{value}</text>
    {/if}
  </svg>
</div>

<style>
  .ring {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    filter: drop-shadow(0 0 4px rgba(0, 255, 102, 0.15));
    transition: filter 0.3s ease;
  }
  .ring:hover {
    filter: drop-shadow(0 0 12px rgba(0, 255, 102, 0.3));
  }
  .progress {
    transition: stroke-dashoffset 0.6s cubic-bezier(0.16, 1, 0.3, 1);
  }
  @media (prefers-reduced-motion: reduce) {
    .ring,
    .progress {
      transition: none;
    }
  }
</style>
