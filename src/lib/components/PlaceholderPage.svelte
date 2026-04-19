<!-- SPDX-License-Identifier: MIT -->
<script lang="ts">
  interface Props {
    title: string;
    phase: string;
  }
  let { title, phase }: Props = $props();
</script>

<div class="flex flex-col items-center justify-center min-h-screen p-8">
  <h1 class="text-4xl font-display text-impforge-neon mb-4">{title}</h1>
  <p class="text-impforge-text-secondary">Coming in {phase}.</p>
  <p class="mt-8 text-sm text-impforge-text-secondary opacity-60">
    ImpForge MIT — open-source side of <a class="text-impforge-cyan underline" href="https://impforge.com">our €25/mo AI workstation</a>.
  </p>
</div>
