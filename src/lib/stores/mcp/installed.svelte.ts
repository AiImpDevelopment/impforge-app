// SPDX-License-Identifier: MIT
// Installed-server store — drives InstalledView + ConfigDialog.

import { invoke } from '@tauri-apps/api/core';
import type { Transport } from './marketplace.svelte';

export interface InstalledServer {
  id: string;
  version: string;
  installed_at: string;
  config_path: string;
  transport: Transport;
  command: string | null;
  args: string[];
  env: Record<string, string>;
  tool_allowlist: string[] | null;
  fs_allowlist: string[];
  net_allowlist: string[];
  enabled: boolean;
  bundle_source: string | null;
}

export interface InstallResult {
  installed: InstalledServer;
  seeded_keyring_count: number;
  warnings: string[];
}

export interface InstallRequest {
  server_id: string;
  version?: string;
  env: Record<string, string>;
  fs_allowlist: string[];
  net_allowlist: string[];
  bundle_source?: string;
}

interface InstalledState {
  servers: InstalledServer[];
  loading: boolean;
  lastError: string | null;
  installCount: number;
}

export const installed = $state<InstalledState>({
  servers: [],
  loading: false,
  lastError: null,
  installCount: 0,
});

export async function refreshInstalled(): Promise<void> {
  installed.loading = true;
  installed.lastError = null;
  try {
    installed.servers = await invoke<InstalledServer[]>('mcp_list_installed');
    installed.installCount = installed.servers.length;
  } catch (err) {
    installed.lastError = String(err);
    installed.servers = [];
  } finally {
    installed.loading = false;
  }
}

export async function installServer(req: InstallRequest): Promise<InstallResult | null> {
  installed.loading = true;
  installed.lastError = null;
  try {
    const result = await invoke<InstallResult>('mcp_install_server', { req });
    await refreshInstalled();
    return result;
  } catch (err) {
    installed.lastError = String(err);
    return null;
  } finally {
    installed.loading = false;
  }
}

export async function uninstallServer(id: string): Promise<boolean> {
  installed.loading = true;
  installed.lastError = null;
  try {
    await invoke('mcp_uninstall_server', { id });
    await refreshInstalled();
    return true;
  } catch (err) {
    installed.lastError = String(err);
    return false;
  } finally {
    installed.loading = false;
  }
}

export async function setEnabled(id: string, enabled: boolean): Promise<boolean> {
  try {
    await invoke<InstalledServer>('mcp_set_enabled', { id, enabled });
    await refreshInstalled();
    return true;
  } catch (err) {
    installed.lastError = String(err);
    return false;
  }
}

export async function pinVersion(id: string, version: string): Promise<boolean> {
  try {
    await invoke<InstalledServer>('mcp_pin_version', { id, version });
    await refreshInstalled();
    return true;
  } catch (err) {
    installed.lastError = String(err);
    return false;
  }
}

export async function setToolAllowlist(
  id: string,
  allowlist: string[] | null,
): Promise<boolean> {
  try {
    await invoke<InstalledServer>('mcp_set_tool_allowlist', { id, allowlist });
    await refreshInstalled();
    return true;
  } catch (err) {
    installed.lastError = String(err);
    return false;
  }
}
