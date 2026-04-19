<!-- SPDX-License-Identifier: MIT -->
<!--
  Render a JSON-Schema-style input schema as a visual list of fields.
  Used by DetailView to show every parameter a tool accepts BEFORE
  the user installs the server (Invariant Labs anti-truncation defence).
-->
<script lang="ts">
  interface Props {
    schema: Record<string, unknown> | undefined | null;
    name?: string;
  }

  let { schema, name = 'parameters' }: Props = $props();

  interface Field {
    name: string;
    type: string;
    required: boolean;
    description: string;
    enumValues?: string[];
  }

  const fields = $derived.by(() => {
    if (!schema || typeof schema !== 'object') return [] as Field[];
    const props = (schema as Record<string, unknown>).properties as Record<string, Record<string, unknown>> | undefined;
    if (!props) return [] as Field[];
    const required = ((schema as Record<string, unknown>).required as string[]) ?? [];
    const out: Field[] = [];
    for (const [k, v] of Object.entries(props)) {
      out.push({
        name: k,
        type: typeof v.type === 'string' ? (v.type as string) : 'any',
        required: required.includes(k),
        description: typeof v.description === 'string' ? (v.description as string) : '',
        enumValues: Array.isArray(v.enum) ? (v.enum as string[]) : undefined,
      });
    }
    return out;
  });
</script>

{#if fields.length === 0}
  <p class="empty">{name}: <em>(none)</em></p>
{:else}
  <div class="schema">
    {#each fields as f (f.name)}
      <div class="field">
        <div class="field-head">
          <code class="name mono">{f.name}</code>
          <code class="type mono">{f.type}</code>
          {#if f.required}
            <span class="badge required">required</span>
          {/if}
        </div>
        {#if f.description}
          <p class="desc">{f.description}</p>
        {/if}
        {#if f.enumValues}
          <div class="enum-row">
            {#each f.enumValues as v}
              <code class="enum-value mono">{v}</code>
            {/each}
          </div>
        {/if}
      </div>
    {/each}
  </div>
{/if}

<style>
  .empty {
    color: #606070;
    font-family: 'Inter', sans-serif;
    font-size: 12px;
    margin: 0;
  }
  .schema {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .field {
    padding: 10px 12px;
    background: rgba(19, 19, 26, 0.6);
    border: 1px solid #1F1F2A;
    border-radius: 8px;
  }
  .field-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .name {
    color: #00FF66;
    font-size: 12px;
    font-weight: 600;
  }
  .type {
    color: #00CCFF;
    font-size: 11px;
    background: rgba(0, 204, 255, 0.08);
    padding: 1px 6px;
    border-radius: 4px;
  }
  .badge.required {
    background: rgba(255, 51, 102, 0.12);
    color: #FF3366;
    padding: 1px 6px;
    border-radius: 4px;
    font-family: 'JetBrains Mono', monospace;
    font-size: 10px;
    text-transform: uppercase;
  }
  .desc {
    color: #A0A0B0;
    font-size: 12px;
    margin: 4px 0 0 0;
    line-height: 1.5;
  }
  .enum-row {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 6px;
  }
  .enum-value {
    background: rgba(13, 13, 18, 0.7);
    border: 1px solid #1F1F2A;
    color: #E8E8ED;
    padding: 2px 8px;
    border-radius: 4px;
    font-size: 11px;
  }
  .mono {
    font-family: 'JetBrains Mono', ui-monospace, monospace;
  }
</style>
