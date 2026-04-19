<!-- SPDX-License-Identifier: MIT -->
<!--
  Bento card for one marketplace entry.  Layout obeys ImpForge design
  rules:
    * gradient mesh background (NEVER flat black)
    * neon-glow on hover (NEVER static border)
    * trust signals as cyan icon row
    * capability chips along the bottom
    * Install / Details CTAs

  Stagger entry animation (60ms per index) is applied by the parent
  BrowseView grid via animation-delay on this component.
-->
<script lang="ts">
  import type { MarketplaceEntry, TrustScore } from '$lib/stores/mcp/marketplace.svelte';
  import { getTrustScore } from '$lib/stores/mcp/marketplace.svelte';
  import TrustScoreRing from './TrustScoreRing.svelte';
  import CapabilityChip from './CapabilityChip.svelte';

  interface Props {
    entry: MarketplaceEntry;
    index?: number;
    onInstall?: (id: string) => void;
    onDetails?: (id: string) => void;
  }

  let { entry, index = 0, onInstall, onDetails }: Props = $props();

  let trust = $state<TrustScore | null>(null);
  $effect(() => {
    void getTrustScore(entry.id).then((s) => {
      trust = s;
    });
  });

  const installs = $derived.by(() => {
    if (entry.install_count >= 1_000_000) {
      return `${(entry.install_count / 1_000_000).toFixed(1)}M`;
    }
    if (entry.install_count >= 1_000) {
      return `${(entry.install_count / 1_000).toFixed(1)}K`;
    }
    return entry.install_count.toString();
  });

  const lastUpdated = $derived.by(() => {
    const dt = new Date(entry.last_updated);
    if (Number.isNaN(dt.getTime())) return '';
    const days = Math.floor((Date.now() - dt.getTime()) / (1000 * 60 * 60 * 24));
    if (days < 1) return 'today';
    if (days < 30) return `${days}d ago`;
    if (days < 365) return `${Math.floor(days / 30)}mo ago`;
    return `${Math.floor(days / 365)}y ago`;
  });
</script>

<article class="card" style="animation-delay: {index * 60}ms">
  <header>
    <div class="title-row">
      <h3 class="name">{entry.name}</h3>
      {#if entry.verified}
        <span class="badge verified" title="Verified publisher">
          <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
            <path
              d="M8 1l1.4 2.7 3 .4-2.2 2.1.5 3-2.7-1.4-2.7 1.4.5-3L3.6 4.1l3-.4z"
              fill="currentColor"
            />
          </svg>
          verified
        </span>
      {/if}
      {#if entry.signed}
        <span class="badge signed" title="Sigstore-signed manifest">
          <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
            <path
              d="M5 7V5a3 3 0 016 0v2h.5a1.5 1.5 0 011.5 1.5v5A1.5 1.5 0 0111.5 15h-7A1.5 1.5 0 013 13.5v-5A1.5 1.5 0 014.5 7H5zm1 0h4V5a2 2 0 00-4 0v2z"
              fill="currentColor"
            />
          </svg>
          signed
        </span>
      {/if}
    </div>
    <p class="publisher">by <span class="publisher-name">{entry.publisher}</span> · v{entry.version}</p>
  </header>

  <p class="description">{entry.description}</p>

  <div class="trust-row">
    <TrustScoreRing score={trust} size={46} />
    <div class="trust-meta">
      <div class="meta-line">
        <span class="meta-label">installs</span>
        <span class="meta-value mono">{installs}</span>
      </div>
      <div class="meta-line">
        <span class="meta-label">stars</span>
        <span class="meta-value mono">{'★'.repeat(entry.stars)}</span>
      </div>
      <div class="meta-line">
        <span class="meta-label">updated</span>
        <span class="meta-value mono">{lastUpdated}</span>
      </div>
    </div>
  </div>

  <div class="chips">
    {#if entry.capability_hint.tools > 0}
      <CapabilityChip label={`tools: ${entry.capability_hint.tools}`} accent="cyan" icon="tools" />
    {/if}
    {#if entry.capability_hint.resources > 0}
      <CapabilityChip label={`resources: ${entry.capability_hint.resources}`} accent="purple" icon="resources" />
    {/if}
    {#if entry.capability_hint.prompts > 0}
      <CapabilityChip label={`prompts: ${entry.capability_hint.prompts}`} accent="magenta" icon="prompts" />
    {/if}
    {#if entry.oauth}
      <CapabilityChip label="oauth" accent="orange" icon="oauth" />
    {/if}
    <CapabilityChip
      label={entry.transport === 'stdio' ? 'local' : 'remote'}
      accent={entry.transport === 'stdio' ? 'neon' : 'magenta'}
      icon={entry.transport === 'stdio' ? 'local' : 'remote'}
    />
  </div>

  <footer>
    <button
      type="button"
      class="cta primary"
      onclick={() => onInstall?.(entry.id)}
      aria-label={`Install ${entry.name}`}
    >
      Install
    </button>
    <button
      type="button"
      class="cta ghost"
      onclick={() => onDetails?.(entry.id)}
      aria-label={`View details for ${entry.name}`}
    >
      Details →
    </button>
  </footer>
</article>

<style>
  .card {
    background:
      linear-gradient(180deg, rgba(0, 255, 102, 0.05) 0%, transparent 100%),
      linear-gradient(135deg, rgba(19, 19, 26, 0.85) 0%, rgba(13, 13, 18, 0.92) 100%);
    border: 1px solid #2A2A3A;
    border-radius: 14px;
    padding: 18px 20px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    transition: all 0.25s cubic-bezier(0.16, 1, 0.3, 1);
    backdrop-filter: blur(16px) saturate(120%);
    will-change: transform;
    animation: enter 0.45s cubic-bezier(0.16, 1, 0.3, 1) backwards;
  }
  .card:hover {
    transform: translateY(-2px);
    border-color: rgba(0, 255, 102, 0.4);
    box-shadow:
      0 0 24px rgba(0, 255, 102, 0.18),
      0 8px 24px rgba(0, 0, 0, 0.4);
  }

  header {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .title-row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .name {
    font-family: 'Space Grotesk', system-ui, sans-serif;
    font-size: 18px;
    font-weight: 600;
    margin: 0;
    color: #E8E8ED;
    letter-spacing: -0.01em;
  }
  .badge {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 8px;
    border-radius: 999px;
    font-family: 'JetBrains Mono', ui-monospace, monospace;
    font-size: 10px;
    line-height: 1.4;
    border: 1px solid currentColor;
    background: rgba(19, 19, 26, 0.4);
  }
  .badge.verified {
    color: #00FF66;
  }
  .badge.signed {
    color: #00CCFF;
  }
  .publisher {
    margin: 0;
    color: #A0A0B0;
    font-size: 12px;
    font-family: 'Inter', system-ui, sans-serif;
  }
  .publisher-name {
    color: #E8E8ED;
    font-weight: 500;
  }

  .description {
    margin: 0;
    color: #A0A0B0;
    font-family: 'Inter', system-ui, sans-serif;
    font-size: 13px;
    line-height: 1.5;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .trust-row {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .trust-meta {
    display: flex;
    flex-direction: column;
    gap: 3px;
    flex: 1;
  }
  .meta-line {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 11px;
  }
  .meta-label {
    color: #606070;
    font-family: 'Inter', sans-serif;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .meta-value {
    color: #E8E8ED;
  }
  .mono {
    font-family: 'JetBrains Mono', ui-monospace, monospace;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  footer {
    display: flex;
    gap: 10px;
    margin-top: 4px;
  }
  .cta {
    flex: 1;
    padding: 8px 14px;
    border-radius: 8px;
    font-family: 'Space Grotesk', system-ui, sans-serif;
    font-weight: 600;
    font-size: 13px;
    cursor: pointer;
    transition: all 0.2s ease;
    border: 1px solid transparent;
  }
  .cta.primary {
    background: linear-gradient(135deg, #00FF66 0%, #00CC52 100%);
    color: #0D0D12;
    border-color: #00FF66;
  }
  .cta.primary:hover {
    box-shadow: 0 0 16px rgba(0, 255, 102, 0.4);
    transform: translateY(-1px);
  }
  .cta.ghost {
    background: transparent;
    color: #00FF66;
    border-color: rgba(0, 255, 102, 0.4);
  }
  .cta.ghost:hover {
    border-color: #00FF66;
    background: rgba(0, 255, 102, 0.06);
  }

  @keyframes enter {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .card,
    .cta {
      animation: none;
      transition: none;
    }
  }
</style>
