<!-- SPDX-License-Identifier: MIT -->
<!--
  ProviderSettings — full Settings → Providers panel.

  One card per provider:
    - status pill (key set / no key / local)
    - password input + Save / Remove buttons
    - per-provider running spend (call count + bytes + avg latency)
    - "Powered by <provider>" attribution badge per their ToS

  Spend is mirrored from `spend.svelte.ts` (read-only — never phones home).
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import {
    getProvidersState,
    refreshProviders,
    addProviderKey,
    removeProviderKey,
    type ProviderId,
    type ProviderInfo
  } from '$lib/stores/providers.svelte';
  import {
    getSpendState,
    refreshSpend,
    resetSpend,
    type ProviderSpend
  } from '$lib/stores/spend.svelte';

  const providers = $derived(getProvidersState());
  const spend = $derived(getSpendState());

  // Per-card transient input state — keys never persist outside the OS keychain.
  let keyDrafts = $state<Record<string, string>>({});
  let busy = $state<Record<string, boolean>>({});

  onMount(() => {
    void refreshProviders();
    void refreshSpend();
  });

  function spendFor(id: ProviderId): ProviderSpend | undefined {
    return spend.spend.find((s) => s.provider === id);
  }

  function avgLatency(s: ProviderSpend): number {
    if (s.call_count === 0) return 0;
    return Math.round(s.total_latency_ms / s.call_count);
  }

  function fmtBytes(b: number): string {
    if (b === 0) return '0 B';
    if (b < 1024) return `${b} B`;
    if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`;
    return `${(b / (1024 * 1024)).toFixed(2)} MB`;
  }

  async function onSave(p: ProviderInfo): Promise<void> {
    const draft = keyDrafts[p.id]?.trim() ?? '';
    if (!draft) return;
    busy[p.id] = true;
    try {
      await addProviderKey(p.id, draft);
      keyDrafts[p.id] = '';
    } catch {
      // store handles lastError display
    } finally {
      busy[p.id] = false;
    }
  }

  async function onRemove(p: ProviderInfo): Promise<void> {
    busy[p.id] = true;
    try {
      await removeProviderKey(p.id);
    } catch {
      // store handles lastError display
    } finally {
      busy[p.id] = false;
    }
  }

  async function onResetSpend(): Promise<void> {
    await resetSpend();
  }

  function statusLabel(p: ProviderInfo): string {
    if (!p.requires_key) return 'local-only';
    return p.key_status === 'set' ? 'key set' : 'no key';
  }

  function statusClass(p: ProviderInfo): string {
    if (!p.requires_key) return 'text-impforge-cyan border-impforge-cyan/40 bg-impforge-cyan/10';
    return p.key_status === 'set'
      ? 'text-impforge-neon border-impforge-neon/40 bg-impforge-neon/10'
      : 'text-impforge-magenta border-impforge-magenta/40 bg-impforge-magenta/10';
  }
</script>

<section class="space-y-4 px-6 py-6">
  <header class="flex items-center justify-between">
    <div>
      <h2 class="font-display text-2xl font-bold text-impforge-neon">AI Providers</h2>
      <p class="mt-1 text-sm text-impforge-text-secondary">
        Bring your own key — your prompts go directly to the provider you choose.
        Keys are stored in the OS keychain (Secret Service / Keychain / Credential Manager)
        and never leave your machine.
      </p>
    </div>
    <button
      type="button"
      onclick={onResetSpend}
      class="rounded-md border border-impforge-border bg-impforge-bg-secondary px-3 py-1.5 font-mono text-xs uppercase tracking-wider text-impforge-text-secondary transition-colors hover:border-impforge-magenta hover:text-impforge-magenta focus:outline-none focus:ring-2 focus:ring-impforge-magenta"
    >
      Reset spend
    </button>
  </header>

  {#if providers.lastError}
    <div class="rounded-md border border-red-700/50 bg-red-950/30 px-3 py-2">
      <p class="font-mono text-xs uppercase tracking-wider text-red-400">error</p>
      <p class="mt-1 text-xs text-red-200">{providers.lastError}</p>
    </div>
  {/if}

  <div class="grid gap-4 lg:grid-cols-2">
    {#each providers.providers as p (p.id)}
      {@const sp = spendFor(p.id)}
      <article
        class="rounded-xl border border-impforge-border bg-impforge-bg-secondary p-4 shadow-sm transition-colors hover:border-impforge-neon/40"
      >
        <header class="mb-3 flex items-center justify-between">
          <div class="flex items-center gap-2">
            <h3 class="font-display text-lg font-semibold text-impforge-text-primary">
              {p.display_name}
            </h3>
            <span
              class="rounded-md border px-2 py-0.5 font-mono text-[10px] uppercase tracking-wider {statusClass(
                p
              )}"
            >
              {statusLabel(p)}
            </span>
          </div>
          <span class="font-mono text-[10px] text-impforge-text-secondary">
            {p.default_model}
          </span>
        </header>

        {#if p.requires_key}
          <div class="mb-3 flex gap-2">
            <input
              type="password"
              bind:value={keyDrafts[p.id]}
              placeholder={p.key_status === 'set'
                ? 'enter new key to replace'
                : 'paste API key'}
              disabled={busy[p.id]}
              class="flex-1 rounded-md border border-impforge-border bg-impforge-bg-primary px-2 py-1.5 font-mono text-xs text-impforge-text-primary placeholder:text-impforge-text-secondary focus:border-impforge-neon focus:outline-none disabled:opacity-50"
            />
            <button
              type="button"
              onclick={() => onSave(p)}
              disabled={busy[p.id] || !keyDrafts[p.id]?.trim()}
              class="rounded-md border border-impforge-neon bg-impforge-neon px-3 py-1.5 text-xs font-semibold text-impforge-bg-primary transition-all hover:shadow-[0_0_16px_rgba(0,255,102,0.3)] disabled:cursor-not-allowed disabled:opacity-50"
            >
              Save
            </button>
            {#if p.key_status === 'set'}
              <button
                type="button"
                onclick={() => onRemove(p)}
                disabled={busy[p.id]}
                class="rounded-md border border-impforge-border bg-impforge-bg-primary px-3 py-1.5 font-mono text-xs uppercase tracking-wider text-impforge-text-secondary transition-colors hover:border-impforge-magenta hover:text-impforge-magenta disabled:opacity-50"
              >
                Remove
              </button>
            {/if}
          </div>
        {:else}
          <p class="mb-3 text-xs text-impforge-text-secondary">
            Local-only — talks to <code class="font-mono text-impforge-cyan">localhost:11434</code>.
          </p>
        {/if}

        <dl class="grid grid-cols-3 gap-2 border-t border-impforge-border pt-3 font-mono text-[11px]">
          <div>
            <dt class="uppercase tracking-wider text-impforge-text-secondary">calls</dt>
            <dd class="mt-0.5 text-impforge-text-primary">
              {sp?.call_count ?? 0}
            </dd>
          </div>
          <div>
            <dt class="uppercase tracking-wider text-impforge-text-secondary">bytes</dt>
            <dd class="mt-0.5 text-impforge-text-primary">
              {fmtBytes(sp?.total_bytes ?? 0)}
            </dd>
          </div>
          <div>
            <dt class="uppercase tracking-wider text-impforge-text-secondary">avg latency</dt>
            <dd class="mt-0.5 text-impforge-text-primary">
              {sp ? `${avgLatency(sp)} ms` : '—'}
            </dd>
          </div>
        </dl>

        <p class="mt-3 text-[10px] text-impforge-text-secondary">
          Powered by {p.display_name} — your prompts go directly to their API.
          ImpForge never sees them.
        </p>
      </article>
    {/each}
  </div>
</section>
