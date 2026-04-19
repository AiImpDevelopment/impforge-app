<!-- SPDX-License-Identifier: MIT -->
<!--
  ProviderPicker — compact provider+model dropdown for the chat header.

  - Reactive on the providers store
  - Disables remote providers when no key is stored, with a tooltip linking
    to Settings → Providers
  - Calls `selectProvider`/`selectModel` so other components $derive from
    the same source of truth
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import {
    getProvidersState,
    refreshProviders,
    selectProvider,
    selectModel,
    type ProviderId
  } from '$lib/stores/providers.svelte';

  const providers = $derived(getProvidersState());

  onMount(() => {
    void refreshProviders();
  });

  function onProviderChange(e: Event): void {
    const target = e.target as HTMLSelectElement;
    selectProvider(target.value as ProviderId);
  }

  function onModelChange(e: Event): void {
    const target = e.target as HTMLInputElement;
    selectModel(target.value);
  }

  function statusBadge(status: 'set' | 'missing' | 'local'): string {
    switch (status) {
      case 'set':
        return 'text-impforge-neon';
      case 'local':
        return 'text-impforge-cyan';
      case 'missing':
        return 'text-impforge-magenta';
    }
  }
</script>

<div class="flex items-center gap-2">
  <label for="provider-picker" class="font-mono text-[11px] uppercase tracking-wider text-impforge-text-secondary">
    provider
  </label>
  <select
    id="provider-picker"
    value={providers.selectedProviderId}
    onchange={onProviderChange}
    class="rounded-md border border-impforge-border bg-impforge-bg-secondary px-2 py-1 text-xs text-impforge-text-primary focus:border-impforge-neon focus:outline-none"
  >
    {#each providers.providers as p (p.id)}
      <option
        value={p.id}
        disabled={p.requires_key && p.key_status !== 'set'}
      >
        {p.display_name} {p.requires_key && p.key_status !== 'set' ? '(no key)' : ''}
      </option>
    {/each}
  </select>

  <input
    type="text"
    value={providers.selectedModel}
    oninput={onModelChange}
    placeholder="model"
    class="w-44 rounded-md border border-impforge-border bg-impforge-bg-secondary px-2 py-1 font-mono text-xs text-impforge-text-primary focus:border-impforge-neon focus:outline-none"
  />

  {#if providers.providers.length > 0}
    {@const cur = providers.providers.find((p) => p.id === providers.selectedProviderId)}
    {#if cur}
      <span class="font-mono text-[10px] uppercase tracking-wider {statusBadge(cur.key_status)}">
        {cur.key_status}
      </span>
    {/if}
  {/if}

  {#if providers.lastError}
    <span class="font-mono text-[10px] text-red-400" title={providers.lastError}>
      err
    </span>
  {/if}
</div>
