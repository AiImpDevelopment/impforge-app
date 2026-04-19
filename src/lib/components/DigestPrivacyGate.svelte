<!-- SPDX-License-Identifier: MIT -->
<script lang="ts">
  import {
    getDigestState,
    pause,
    resume,
    setQuietHours,
    refreshAll,
  } from '$lib/stores/digest.svelte';

  const digest = $derived(getDigestState());

  let busy = $state(false);
  let error = $state<string | null>(null);

  // Local controls bound to the store; these mirror digest.stats so the
  // user always sees the persisted values.
  const paused = $derived(digest.stats?.paused ?? false);
  const quietEnabled = $derived(digest.stats?.quiet_hours_enabled ?? true);

  let quietStart = $state(22);
  let quietEnd = $state(8);

  async function togglePause() {
    busy = true;
    error = null;
    try {
      if (paused) {
        await resume();
      } else {
        await pause();
      }
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function applyQuietHours(enabled: boolean) {
    busy = true;
    error = null;
    try {
      await setQuietHours(enabled, quietStart, quietEnd);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  $effect(() => {
    refreshAll();
  });
</script>

<aside class="privacy-gate">
  <header>
    <h2>Privacy controls</h2>
  </header>

  {#if error}
    <div class="error">{error}</div>
  {/if}

  <!-- Master pause -->
  <article class="control-card primary">
    <div class="control-header">
      <h3>Pause All Digest</h3>
      <span class="status {paused ? 'paused' : 'live'}">
        {paused ? '⏸ paused' : '● live'}
      </span>
    </div>
    <p class="hint">
      Single-click stop for every digest source. Resume any time —
      your sources stay configured.
    </p>
    <button class="big" onclick={togglePause} disabled={busy}>
      {paused ? 'Resume digest' : 'Pause everything'}
    </button>
  </article>

  <!-- Quiet hours -->
  <article class="control-card">
    <div class="control-header">
      <h3>Quiet hours</h3>
      <span class="status {quietEnabled ? 'live' : 'paused'}">
        {quietEnabled ? '● enabled' : '○ off'}
      </span>
    </div>
    <p class="hint">
      Suppress feed polling between {quietStart}:00 and {quietEnd}:00 local
      time so the daemon stays quiet at night.
    </p>
    <div class="row">
      <label>
        Start
        <input
          type="number"
          min="0"
          max="23"
          bind:value={quietStart}
        />
      </label>
      <label>
        End
        <input
          type="number"
          min="0"
          max="23"
          bind:value={quietEnd}
        />
      </label>
      <button onclick={() => applyQuietHours(!quietEnabled)} disabled={busy}>
        {quietEnabled ? 'Disable quiet hours' : 'Enable quiet hours'}
      </button>
      <button onclick={() => applyQuietHours(true)} disabled={busy}>
        Save
      </button>
    </div>
  </article>

  <!-- Recall anti-pattern table — surfaces our defenses to the user -->
  <article class="control-card">
    <h3>How we differ from Microsoft Recall</h3>
    <table class="anti-recall">
      <thead>
        <tr><th>Pattern</th><th>Recall</th><th>ImpForge</th></tr>
      </thead>
      <tbody>
        <tr>
          <td>Default state</td>
          <td>On</td>
          <td><strong>Off — every source opt-in</strong></td>
        </tr>
        <tr>
          <td>Encryption at rest</td>
          <td>Plaintext SQLite</td>
          <td><strong>AES-GCM via crypto_lite</strong></td>
        </tr>
        <tr>
          <td>OCR scope</td>
          <td>Auto, every screen</td>
          <td><strong>Opt-in folder-only</strong></td>
        </tr>
        <tr>
          <td>PII filter</td>
          <td>None</td>
          <td><strong>Pre-storage scrub</strong></td>
        </tr>
        <tr>
          <td>Provenance trail</td>
          <td>Limited</td>
          <td><strong>Source URL/path on every entry</strong></td>
        </tr>
        <tr>
          <td>Cloud sync</td>
          <td>OneDrive default</td>
          <td><strong>Local-only (Pro adds opt-in cloud)</strong></td>
        </tr>
        <tr>
          <td>Pause</td>
          <td>Buried in settings</td>
          <td><strong>One-click pause All</strong></td>
        </tr>
        <tr>
          <td>Quiet hours</td>
          <td>n/a</td>
          <td><strong>22:00-08:00 local default</strong></td>
        </tr>
        <tr>
          <td>App / folder gating</td>
          <td>Limited per-app</td>
          <td><strong>Per-folder allow_ext list</strong></td>
        </tr>
        <tr>
          <td>Telemetry</td>
          <td>Microsoft cloud</td>
          <td><strong>Zero telemetry</strong></td>
        </tr>
        <tr>
          <td>Audit log</td>
          <td>None visible</td>
          <td><strong>Activity feed in this UI</strong></td>
        </tr>
      </tbody>
    </table>
  </article>
</aside>

<style>
  .privacy-gate {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    padding: 1.5rem;
    color: #e8e8ed;
  }
  h2 {
    margin: 0;
    font-family: 'Space Grotesk', sans-serif;
  }
  .control-card {
    background: #13131a;
    border: 1px solid #2a2a3a;
    border-radius: 8px;
    padding: 1rem;
  }
  .control-card.primary {
    border-left: 3px solid #00ff66;
  }
  .control-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.5rem;
  }
  .control-header h3 {
    margin: 0;
    font-family: 'Space Grotesk', sans-serif;
  }
  .status {
    font-size: 0.8rem;
    padding: 0.2rem 0.6rem;
    border-radius: 999px;
    font-weight: 600;
  }
  .status.live {
    background: rgba(0, 255, 102, 0.15);
    color: #00ff66;
  }
  .status.paused {
    background: rgba(255, 170, 0, 0.15);
    color: #ffaa00;
  }
  .hint {
    color: #a0a0b0;
    font-size: 0.85rem;
    line-height: 1.5;
    margin: 0 0 0.75rem;
  }
  .row {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
    align-items: center;
  }
  label {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.85rem;
    color: #a0a0b0;
  }
  input[type='number'] {
    width: 70px;
    padding: 0.4rem;
    background: #08080d;
    border: 1px solid #2a2a3a;
    border-radius: 4px;
    color: #e8e8ed;
  }
  button {
    padding: 0.5rem 1rem;
    background: #00ff66;
    color: #08080d;
    border: 1px solid #00ff66;
    border-radius: 4px;
    cursor: pointer;
    font-weight: 600;
    font-size: 0.85rem;
  }
  button.big {
    padding: 0.75rem 1.5rem;
    width: 100%;
    font-size: 1rem;
  }
  button:hover:not(:disabled) {
    box-shadow: 0 0 16px rgba(0, 255, 102, 0.4);
  }
  button:disabled {
    background: #2a2a3a;
    color: #a0a0b0;
    cursor: not-allowed;
  }
  .anti-recall {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.85rem;
    margin-top: 0.5rem;
  }
  .anti-recall th,
  .anti-recall td {
    padding: 0.4rem 0.6rem;
    text-align: left;
    border-bottom: 1px solid #2a2a3a;
  }
  .anti-recall th {
    background: #08080d;
    color: #00ccff;
    font-weight: 600;
    font-family: 'JetBrains Mono', monospace;
  }
  .anti-recall td:nth-child(2) {
    color: #ff8888;
  }
  .anti-recall td:nth-child(3) {
    color: #00ff66;
  }
  .error {
    background: rgba(255, 51, 51, 0.1);
    border: 1px solid rgba(255, 51, 51, 0.3);
    border-radius: 6px;
    color: #ff8888;
    padding: 0.75rem 1rem;
  }
</style>
