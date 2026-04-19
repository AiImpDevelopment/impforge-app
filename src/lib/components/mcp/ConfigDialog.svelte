<!-- SPDX-License-Identifier: MIT -->
<!--
  Modal config dialog — env vars + paths + OAuth.  Renders fields from
  the server's `env_schema` JSON-Schema.  Secret fields (PAT / API key)
  show a masked input with a reveal toggle; required fields block the
  Install CTA until populated.

  OAuth flow: the user clicks "Connect with <Provider>", which kicks off
  `mcp_oauth_start` and opens the system browser to the authorize URL.
-->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import type { MarketplaceEntry } from '$lib/stores/mcp/marketplace.svelte';

  interface Props {
    entry: MarketplaceEntry;
    open: boolean;
    onClose?: () => void;
    onSubmit?: (env: Record<string, string>, fsPaths: string[], netHosts: string[]) => void;
  }

  let { entry, open, onClose, onSubmit }: Props = $props();

  let env = $state<Record<string, string>>({});
  let revealed = $state<Record<string, boolean>>({});
  let fsAllowlistRaw = $state('');
  let netAllowlistRaw = $state('');
  let oauthInProgress = $state(false);
  let oauthError = $state<string | null>(null);

  $effect(() => {
    if (open) {
      env = {};
      revealed = {};
      fsAllowlistRaw = '';
      netAllowlistRaw = '';
      oauthError = null;
    }
  });

  const allRequiredFilled = $derived.by(() => {
    if (!entry?.env_schema) return true;
    return entry.env_schema
      .filter((s) => s.required)
      .every((s) => (env[s.name] ?? '').trim().length > 0);
  });

  function toggleReveal(name: string): void {
    revealed[name] = !revealed[name];
  }

  function close(): void {
    onClose?.();
  }

  async function startOauth(): Promise<void> {
    oauthInProgress = true;
    oauthError = null;
    try {
      const provider = entry.tags.find((t) => t === 'oauth')
        ? (entry.id.includes('github') ? 'github' : entry.id.includes('slack') ? 'slack' : 'google')
        : 'github';
      const flow = await invoke<{ authorize_url: string }>('mcp_oauth_start', {
        req: {
          server_id: entry.id,
          provider,
          client_id: env['CLIENT_ID'] ?? '',
          redirect_port: 47823,
          scopes: null,
        },
      });
      // Open the browser via Tauri's open shell — fall back to a text
      // clipboard hint if not available.
      window.open(flow.authorize_url, '_blank');
    } catch (err) {
      oauthError = String(err);
    } finally {
      oauthInProgress = false;
    }
  }

  function submit(): void {
    const fsPaths = fsAllowlistRaw
      .split('\n')
      .map((s) => s.trim())
      .filter((s) => s.length > 0);
    const netHosts = netAllowlistRaw
      .split('\n')
      .map((s) => s.trim())
      .filter((s) => s.length > 0);
    onSubmit?.(env, fsPaths, netHosts);
  }
</script>

{#if open}
  <div
    class="overlay"
    role="dialog"
    aria-modal="true"
    aria-labelledby="dialog-title"
    tabindex="-1"
    onclick={close}
    onkeydown={(e) => e.key === 'Escape' && close()}
  >
    <div class="dialog" onclick={(e) => e.stopPropagation()} role="presentation">
      <header>
        <h2 id="dialog-title">Configure {entry.name}</h2>
        <button type="button" class="close" onclick={close} aria-label="Close dialog">×</button>
      </header>

      <p class="hint">Pre-flight check before install.  Secret values stay in your OS keychain — never written to disk in plaintext.</p>

      {#if entry.env_schema && entry.env_schema.length > 0}
        <section class="block">
          <h3>Environment</h3>
          {#each entry.env_schema as spec (spec.name)}
            <label class="field">
              <span class="field-label">
                <code class="mono">{spec.name}</code>
                {#if spec.required}
                  <span class="badge required">required</span>
                {/if}
                {#if spec.secret}
                  <span class="badge secret">secret</span>
                {/if}
              </span>
              <span class="field-desc">{spec.description}</span>
              <div class="input-row">
                <input
                  type={spec.secret && !revealed[spec.name] ? 'password' : 'text'}
                  bind:value={env[spec.name]}
                  placeholder={spec.secret ? 'paste secret here' : 'value'}
                  class="input mono"
                />
                {#if spec.secret}
                  <button type="button" class="reveal" onclick={() => toggleReveal(spec.name)} aria-label="Toggle reveal">
                    {revealed[spec.name] ? 'hide' : 'show'}
                  </button>
                {/if}
              </div>
            </label>
          {/each}
        </section>
      {/if}

      {#if entry.oauth}
        <section class="block oauth-block">
          <h3>OAuth</h3>
          <p class="oauth-explainer">
            This server uses OAuth.  After connecting, an access token is
            stored in your OS keychain and refreshed automatically.
          </p>
          <button type="button" class="btn oauth" onclick={startOauth} disabled={oauthInProgress}>
            {oauthInProgress ? 'Opening browser…' : `Connect with ${entry.name}`}
          </button>
          {#if oauthError}
            <p class="error mono">{oauthError}</p>
          {/if}
        </section>
      {/if}

      <section class="block">
        <h3>Filesystem allow-list</h3>
        <p class="hint">One path per line.  The server can only read/write inside these directories.</p>
        <textarea
          class="textarea mono"
          placeholder="/home/me/projects&#10;/tmp/scratch"
          bind:value={fsAllowlistRaw}
          rows="3"
        ></textarea>
      </section>

      <section class="block">
        <h3>Network allow-list</h3>
        <p class="hint">One host per line.  Use <code class="mono">*.example.com</code> for subdomains, <code class="mono">*</code> for any host.</p>
        <textarea
          class="textarea mono"
          placeholder="api.example.com&#10;*.staging.example.com"
          bind:value={netAllowlistRaw}
          rows="3"
        ></textarea>
      </section>

      <footer>
        <button type="button" class="btn ghost" onclick={close}>Cancel</button>
        <button
          type="button"
          class="btn primary"
          onclick={submit}
          disabled={!allRequiredFilled}
          title={!allRequiredFilled ? 'Fill required fields first' : 'Install with this config'}
        >
          Install
        </button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(8, 8, 13, 0.78);
    backdrop-filter: blur(8px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    animation: fade 0.2s ease;
  }
  .dialog {
    background: linear-gradient(135deg, rgba(19, 19, 26, 0.95) 0%, rgba(13, 13, 18, 0.98) 100%);
    border: 1px solid #2A2A3A;
    border-radius: 16px;
    padding: 28px;
    width: min(560px, 92vw);
    max-height: 88vh;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 18px;
    box-shadow:
      0 0 32px rgba(0, 255, 102, 0.1),
      0 16px 48px rgba(0, 0, 0, 0.5);
    animation: slideIn 0.3s cubic-bezier(0.16, 1, 0.3, 1);
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  h2 {
    font-family: 'Space Grotesk', system-ui, sans-serif;
    font-weight: 600;
    margin: 0;
    color: #E8E8ED;
    font-size: 22px;
  }
  .close {
    background: transparent;
    border: none;
    color: #A0A0B0;
    font-size: 28px;
    cursor: pointer;
    line-height: 1;
    padding: 0;
    width: 32px;
    height: 32px;
    border-radius: 6px;
    transition: all 0.2s;
  }
  .close:hover {
    color: #E8E8ED;
    background: rgba(255, 255, 255, 0.06);
  }
  .hint {
    color: #A0A0B0;
    font-family: 'Inter', sans-serif;
    font-size: 12px;
    margin: 0;
  }
  .block {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .block h3 {
    font-family: 'Space Grotesk', sans-serif;
    font-weight: 600;
    font-size: 13px;
    margin: 0;
    color: #E8E8ED;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 10px 12px;
    background: rgba(19, 19, 26, 0.6);
    border: 1px solid #1F1F2A;
    border-radius: 8px;
  }
  .field-label {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: #E8E8ED;
  }
  .field-desc {
    color: #A0A0B0;
    font-family: 'Inter', sans-serif;
    font-size: 12px;
  }
  .badge {
    padding: 1px 6px;
    border-radius: 4px;
    font-size: 10px;
    font-family: 'JetBrains Mono', monospace;
    text-transform: uppercase;
  }
  .badge.required {
    background: rgba(255, 51, 102, 0.12);
    color: #FF3366;
  }
  .badge.secret {
    background: rgba(255, 136, 0, 0.12);
    color: #FF8800;
  }
  .input-row {
    display: flex;
    gap: 6px;
  }
  .input {
    flex: 1;
    background: rgba(8, 8, 13, 0.7);
    border: 1px solid #2A2A3A;
    color: #E8E8ED;
    padding: 8px 10px;
    border-radius: 6px;
    font-size: 13px;
    transition: all 0.2s;
  }
  .input:focus {
    border-color: #00FF66;
    outline: none;
    box-shadow: 0 0 8px rgba(0, 255, 102, 0.2);
  }
  .reveal {
    background: transparent;
    border: 1px solid #2A2A3A;
    color: #A0A0B0;
    padding: 0 12px;
    border-radius: 6px;
    cursor: pointer;
    font-family: 'JetBrains Mono', monospace;
    font-size: 11px;
  }
  .reveal:hover {
    color: #00CCFF;
    border-color: #00CCFF;
  }
  .textarea {
    width: 100%;
    background: rgba(8, 8, 13, 0.7);
    border: 1px solid #2A2A3A;
    color: #E8E8ED;
    padding: 10px;
    border-radius: 6px;
    font-size: 13px;
    line-height: 1.5;
    resize: vertical;
    box-sizing: border-box;
  }
  .textarea:focus {
    border-color: #00FF66;
    outline: none;
    box-shadow: 0 0 8px rgba(0, 255, 102, 0.2);
  }
  .oauth-block {
    background: rgba(255, 136, 0, 0.06);
    border: 1px solid rgba(255, 136, 0, 0.3);
    padding: 14px;
    border-radius: 10px;
  }
  .oauth-explainer {
    color: #A0A0B0;
    font-size: 12px;
    margin: 0;
  }
  .btn.oauth {
    background: linear-gradient(135deg, #FF8800 0%, #CC6600 100%);
    color: #0D0D12;
    padding: 10px 16px;
    border-radius: 8px;
    border: none;
    font-family: 'Space Grotesk', sans-serif;
    font-weight: 600;
    font-size: 13px;
    cursor: pointer;
    transition: all 0.2s;
  }
  .btn.oauth:hover:not(:disabled) {
    box-shadow: 0 0 16px rgba(255, 136, 0, 0.4);
  }
  .btn.oauth:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .error {
    color: #FF3366;
    font-size: 12px;
    margin: 0;
  }
  .mono {
    font-family: 'JetBrains Mono', ui-monospace, monospace;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    padding-top: 12px;
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
    transition: all 0.2s;
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
  .btn.primary:hover:not(:disabled) {
    box-shadow: 0 0 16px rgba(0, 255, 102, 0.4);
  }
  .btn.primary:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  @keyframes fade {
    from { opacity: 0; }
    to { opacity: 1; }
  }
  @keyframes slideIn {
    from {
      opacity: 0;
      transform: translateY(12px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .overlay,
    .dialog {
      animation: none;
    }
  }
</style>
