// SPDX-License-Identifier: MIT
import type { Config } from 'tailwindcss';

export default {
  content: ['./src/**/*.{html,js,svelte,ts}'],
  theme: {
    extend: {
      colors: {
        'impforge-neon': '#00FF66',
        'impforge-cyan': '#00CCFF',
        'impforge-magenta': '#FF3399',
        'impforge-purple': '#9966FF',
        'impforge-bg-void': '#08080D',
        'impforge-bg-primary': '#0D0D12',
        'impforge-bg-secondary': '#13131A',
        'impforge-text-primary': '#E8E8ED',
        'impforge-text-secondary': '#A0A0B0',
        'impforge-border': '#2A2A3A'
      },
      fontFamily: {
        display: ['Space Grotesk', 'system-ui', 'sans-serif'],
        body: ['Inter', 'system-ui', 'sans-serif'],
        mono: ['JetBrains Mono', 'ui-monospace', 'monospace']
      }
    }
  }
} satisfies Config;
