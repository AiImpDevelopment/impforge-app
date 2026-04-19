<!-- SPDX-License-Identifier: MIT -->
<script lang="ts">
  import {
    addFeed,
    addFolder,
    enableClipboard,
    disableClipboard,
    enableScreenshots,
    disableScreenshots,
    defaultScreenshotsFolder,
    importBrowserBookmarks,
    importBrowserHistory,
    refreshAll,
    removeSource,
    getDigestState,
    type DigestSource,
  } from '$lib/stores/digest.svelte';

  const digest = $derived(getDigestState());

  let feedUrl = $state('');
  let feedInterval = $state(300);
  let folderPath = $state('');
  let folderRecursive = $state(true);
  let folderAllowExt = $state('');

  // Inferred from the live state — used in the UI badges.
  const clipboardActive = $derived(digest.clipboardActive);
  const screenshotsActive = $derived(digest.screenshotsActive);

  let busy = $state(false);
  let error = $state<string | null>(null);

  async function handleAddFeed() {
    if (!feedUrl.trim()) return;
    busy = true;
    error = null;
    try {
      await addFeed(feedUrl.trim(), Math.max(feedInterval, 60));
      feedUrl = '';
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function handleAddFolder() {
    if (!folderPath.trim()) return;
    busy = true;
    error = null;
    try {
      const exts = folderAllowExt
        .split(',')
        .map((s) => s.trim())
        .filter(Boolean);
      await addFolder(folderPath.trim(), folderRecursive, exts);
      folderPath = '';
      folderAllowExt = '';
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function handleClipboardToggle() {
    busy = true;
    error = null;
    try {
      if (clipboardActive) {
        await disableClipboard();
      } else {
        await enableClipboard();
      }
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function handleScreenshotsToggle() {
    busy = true;
    error = null;
    try {
      if (screenshotsActive) {
        await disableScreenshots();
      } else {
        const def = await defaultScreenshotsFolder();
        await enableScreenshots(def ?? undefined);
      }
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function handleBrowserImport(family: string, kind: 'bookmarks' | 'history') {
    busy = true;
    error = null;
    try {
      if (kind === 'bookmarks') {
        await importBrowserBookmarks(family);
      } else {
        await importBrowserHistory(family, 30);
      }
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  function describe(src: DigestSource): string {
    switch (src.kind) {
      case 'feed':
        return `feed · ${src.url} · every ${src.interval_secs}s`;
      case 'folder':
        return `folder · ${src.path} · ${src.recursive ? 'recursive' : 'flat'}`;
      case 'clipboard':
        return 'clipboard (opt-in)';
      case 'screenshots':
        return `screenshots · ${src.path ?? '<OS default>'}`;
      case 'browser':
        return `browser · ${src.family}`;
    }
  }

  $effect(() => {
    refreshAll();
  });
</script>

<section class="digest-source-manager">
  <header>
    <h2>Digest Sources</h2>
    <p class="subtitle">
      <strong>Off by default.</strong> Every source is opt-in. Files stay on your disk.
    </p>
  </header>

  {#if error}
    <div class="error">{error}</div>
  {/if}

  <!-- RSS / Atom / JSON-Feed -->
  <article class="source-card">
    <h3>RSS / Atom feed</h3>
    <p class="hint">
      Polls the URL every N seconds (min 60s) and ingests new entries
      with PII scrubbed.
    </p>
    <div class="row">
      <input
        type="url"
        bind:value={feedUrl}
        placeholder="https://example.com/feed.xml"
      />
      <input
        type="number"
        bind:value={feedInterval}
        min="60"
        step="60"
        title="Poll interval (seconds)"
      />
      <button onclick={handleAddFeed} disabled={busy || !feedUrl.trim()}>
        Add feed
      </button>
    </div>
  </article>

  <!-- Local folder -->
  <article class="source-card">
    <h3>Local folder</h3>
    <p class="hint">
      Watches the folder for new / modified files and ingests them.
      Per-folder extension allow-list keeps unwanted files out.
    </p>
    <div class="row">
      <input
        type="text"
        bind:value={folderPath}
        placeholder="/home/you/notes"
      />
      <input
        type="text"
        bind:value={folderAllowExt}
        placeholder="md,txt (comma list, optional)"
      />
      <label>
        <input type="checkbox" bind:checked={folderRecursive} />
        recursive
      </label>
      <button onclick={handleAddFolder} disabled={busy || !folderPath.trim()}>
        Add folder
      </button>
    </div>
  </article>

  <!-- Clipboard (opt-in) -->
  <article class="source-card opt-in">
    <h3>
      Clipboard
      <span class="badge {clipboardActive ? 'on' : 'off'}">
        {clipboardActive ? 'capturing' : 'off'}
      </span>
    </h3>
    <p class="hint">
      Polls the OS clipboard every 750&nbsp;ms when enabled. Each new
      payload runs through the PII scrubber (passwords, API keys,
      credit cards) <strong>before</strong> entering the search index.
    </p>
    <button onclick={handleClipboardToggle} disabled={busy}>
      {clipboardActive ? 'Disable clipboard digest' : 'Enable clipboard digest'}
    </button>
  </article>

  <!-- Screenshots (opt-in, metadata-only at MIT) -->
  <article class="source-card opt-in">
    <h3>
      Screenshots
      <span class="badge {screenshotsActive ? 'on' : 'off'}">
        {screenshotsActive ? 'watching' : 'off'}
      </span>
    </h3>
    <p class="hint">
      Watches your OS screenshot folder. <strong>MIT tier:</strong>
      metadata + filename only. <strong>Pro tier:</strong> full AI OCR
      so screenshot text becomes searchable.
    </p>
    <button onclick={handleScreenshotsToggle} disabled={busy}>
      {screenshotsActive ? 'Disable screenshot digest' : 'Enable screenshot digest'}
    </button>
  </article>

  <!-- Browser bookmarks / history (explicit-action) -->
  <article class="source-card opt-in">
    <h3>Browser bookmarks &amp; history</h3>
    <p class="hint">
      Explicit-action only. Reads via WAL-copy so the browser stays
      open. Pick a family to import.
    </p>
    <div class="row">
      <button onclick={() => handleBrowserImport('firefox', 'bookmarks')} disabled={busy}>
        Import Firefox bookmarks
      </button>
      <button onclick={() => handleBrowserImport('firefox', 'history')} disabled={busy}>
        Last 30d Firefox history
      </button>
      <button onclick={() => handleBrowserImport('chrome', 'bookmarks')} disabled={busy}>
        Import Chrome bookmarks
      </button>
      <button onclick={() => handleBrowserImport('chrome', 'history')} disabled={busy}>
        Last 30d Chrome history
      </button>
    </div>
  </article>

  <!-- Live source list -->
  <article class="source-list">
    <h3>Active sources ({digest.sources.length})</h3>
    {#if digest.sources.length === 0}
      <p class="empty">No sources yet — add one above.</p>
    {:else}
      <ul>
        {#each digest.sources as src (src.id)}
          <li>
            <span class="src-label">{describe(src)}</span>
            <button onclick={() => removeSource(src.id)} disabled={busy}>
              Remove
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </article>
</section>

<style>
  .digest-source-manager {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    padding: 1.5rem;
    color: #e8e8ed;
  }
  .subtitle {
    color: #a0a0b0;
    font-size: 0.9rem;
  }
  .error {
    padding: 0.75rem 1rem;
    background: rgba(255, 51, 51, 0.1);
    border: 1px solid rgba(255, 51, 51, 0.3);
    border-radius: 6px;
    color: #ff8888;
  }
  .source-card {
    padding: 1rem;
    background: #13131a;
    border: 1px solid #2a2a3a;
    border-radius: 8px;
  }
  .source-card.opt-in {
    border-left: 3px solid #00ff66;
  }
  .source-card h3 {
    margin: 0 0 0.5rem;
    font-family: 'Space Grotesk', sans-serif;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }
  .badge {
    font-size: 0.75rem;
    padding: 0.25rem 0.6rem;
    border-radius: 999px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .badge.off {
    background: #2a2a3a;
    color: #a0a0b0;
  }
  .badge.on {
    background: rgba(0, 255, 102, 0.2);
    color: #00ff66;
  }
  .hint {
    color: #a0a0b0;
    font-size: 0.85rem;
    margin: 0 0 0.75rem;
    line-height: 1.5;
  }
  .row {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
    align-items: center;
  }
  .row input[type='text'],
  .row input[type='url'] {
    flex: 1;
    min-width: 200px;
  }
  input,
  button {
    padding: 0.5rem 0.75rem;
    background: #08080d;
    border: 1px solid #2a2a3a;
    border-radius: 4px;
    color: #e8e8ed;
    font-family: inherit;
    font-size: 0.9rem;
  }
  input[type='number'] {
    width: 100px;
  }
  button {
    cursor: pointer;
    background: #00ff66;
    color: #08080d;
    border-color: #00ff66;
    font-weight: 600;
    transition: all 0.2s;
  }
  button:hover:not(:disabled) {
    box-shadow: 0 0 16px rgba(0, 255, 102, 0.4);
    transform: translateY(-1px);
  }
  button:disabled {
    background: #2a2a3a;
    color: #a0a0b0;
    border-color: #2a2a3a;
    cursor: not-allowed;
  }
  label {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.85rem;
    color: #a0a0b0;
  }
  .source-list ul {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .source-list li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.5rem 0.75rem;
    background: #08080d;
    border-radius: 4px;
    font-family: 'JetBrains Mono', monospace;
    font-size: 0.85rem;
  }
  .src-label {
    color: #e8e8ed;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .empty {
    color: #a0a0b0;
    font-style: italic;
  }
</style>
