// SPDX-License-Identifier: MIT
// Settings reactive store — Svelte 5 runes.

interface Settings {
  hyperchatPosition: 'left' | 'right' | 'bottom';
  hyperchatWidth: number;
  digiImpEnabled: boolean;
  digiImpColorPreset: 'neon-green' | 'cyan' | 'magenta' | 'amber';
  digiImpAdventuresEnabled: boolean;
  reducedMotion: boolean;
}

const defaultSettings: Settings = {
  hyperchatPosition: 'right',
  hyperchatWidth: 30,
  digiImpEnabled: true,
  digiImpColorPreset: 'neon-green',
  digiImpAdventuresEnabled: true,
  reducedMotion: false
};

let settings = $state<Settings>(defaultSettings);

export function getSettings(): Settings {
  return settings;
}

export function updateSetting<K extends keyof Settings>(key: K, value: Settings[K]): void {
  settings[key] = value;
}
