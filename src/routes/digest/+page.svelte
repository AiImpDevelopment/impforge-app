<!-- SPDX-License-Identifier: MIT -->
<script lang="ts">
  import DigestSourceManager from '$lib/components/DigestSourceManager.svelte';
  import DigestActivity from '$lib/components/DigestActivity.svelte';
  import DigestPrivacyGate from '$lib/components/DigestPrivacyGate.svelte';
</script>

<div class="digest-page">
  <header class="page-header">
    <h1>Global Digest</h1>
    <p class="subtitle">
      Universal auto-ingest. RSS feeds, watched folders, opt-in clipboard,
      opt-in screenshot folder + explicit-action browser bookmark/history
      import. Everything stays on your disk.
    </p>
  </header>

  <div class="grid">
    <div class="col-main">
      <DigestSourceManager />
      <DigestActivity />
    </div>
    <div class="col-side">
      <DigestPrivacyGate />
    </div>
  </div>
</div>

<style>
  .digest-page {
    min-height: 100%;
    background: linear-gradient(135deg, #0d0d12 0%, #13131a 50%, #0d1117 100%);
    color: #e8e8ed;
  }
  .page-header {
    padding: 2rem 2rem 1rem;
    border-bottom: 1px solid #2a2a3a;
  }
  .page-header h1 {
    margin: 0 0 0.5rem;
    font-family: 'Space Grotesk', sans-serif;
    font-size: 2rem;
    background: linear-gradient(90deg, #00ff66 0%, #00ccff 100%);
    -webkit-background-clip: text;
    background-clip: text;
    -webkit-text-fill-color: transparent;
  }
  .subtitle {
    color: #a0a0b0;
    font-size: 0.95rem;
    line-height: 1.6;
    max-width: 720px;
    margin: 0;
  }
  .grid {
    display: grid;
    grid-template-columns: 2fr 1fr;
    gap: 1rem;
    padding: 1rem 2rem 2rem;
  }
  @media (max-width: 1024px) {
    .grid {
      grid-template-columns: 1fr;
    }
  }
  .col-main {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  .col-side {
    display: flex;
    flex-direction: column;
  }
</style>
