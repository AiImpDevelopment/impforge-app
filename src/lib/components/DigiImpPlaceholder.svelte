<!-- SPDX-License-Identifier: MIT -->
<!--
  DigiImpPlaceholder — Phase 2 stand-in for the real DigiImp visual companion.

  The full pixel-art DigiImp (idle/blink/wake/rest sprites + audio layer)
  lands in Phase 4 per `docs/superpowers/specs/2026-04-19-digiimp-visual-companion-design.md`.

  For the v0.2.0-alpha milestone we render a calm purple pulsing orb in the
  top-right of HyperChatPanel so users get a sense of where the companion
  will live. The pulse honours `prefers-reduced-motion` via the global rule
  in `src/app.css`.
-->
<script lang="ts">
  interface Props {
    /** Optional positioning override; default top-right of the chat panel. */
    position?: 'top-right' | 'top-left';
  }

  const { position = 'top-right' }: Props = $props();
</script>

<div
  class="absolute z-10"
  class:top-4={true}
  class:right-4={position === 'top-right'}
  class:left-4={position === 'top-left'}
  title="DigiImp (Phase 4 — visual companion lands later)"
  aria-label="DigiImp placeholder"
>
  <div
    class="relative h-12 w-12 rounded-full border-2 border-impforge-purple bg-impforge-purple/20"
    style="animation: digiimp-pulse 3000ms cubic-bezier(0.4, 0, 0.6, 1) infinite;"
  >
    <div
      class="absolute inset-2 rounded-full bg-impforge-purple/40"
      style="filter: blur(4px);"
    ></div>
  </div>
</div>

<style>
  @keyframes digiimp-pulse {
    0%,
    100% {
      box-shadow:
        0 0 12px rgba(153, 102, 255, 0.25),
        0 0 24px rgba(153, 102, 255, 0.1);
      transform: scale(1);
    }
    50% {
      box-shadow:
        0 0 24px rgba(153, 102, 255, 0.45),
        0 0 48px rgba(153, 102, 255, 0.2);
      transform: scale(1.04);
    }
  }
</style>
