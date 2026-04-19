// SPDX-License-Identifier: MIT
// Health-dashboard store — polled every 5s while the route is mounted.

import { invoke } from '@tauri-apps/api/core';
import type { InstalledServer } from './installed.svelte';

export interface ServerHealth {
  server_id: string;
  running: boolean;
  p95_latency_ms: number | null;
  success_rate: number;
  calls_last_hour: number;
  restart_count: number;
  last_error: string | null;
  uptime_seconds: number;
}

export interface HealthRow {
  server: InstalledServer;
  health: ServerHealth;
  running: boolean;
  last_started: string | null;
}

export interface LedgerVerification {
  status: 'ok' | 'tampered' | 'empty';
  rows?: number;
  broken_seq?: number;
  reason?: string;
}

export interface LedgerEntry {
  seq: number;
  timestamp: string;
  server_id: string;
  tool_name: string;
  args_hash: string;
  result_hash: string;
  prev_hash: string;
  row_hash: string;
  success: boolean;
  latency_ms: number;
}

interface HealthState {
  rows: HealthRow[];
  loading: boolean;
  lastError: string | null;
  ledgerStatus: LedgerVerification | null;
  ledgerRows: LedgerEntry[];
  pollIntervalMs: number;
  pollTimer: ReturnType<typeof setInterval> | null;
}

export const health = $state<HealthState>({
  rows: [],
  loading: false,
  lastError: null,
  ledgerStatus: null,
  ledgerRows: [],
  pollIntervalMs: 5000,
  pollTimer: null,
});

export async function refreshDashboard(): Promise<void> {
  health.loading = true;
  health.lastError = null;
  try {
    health.rows = await invoke<HealthRow[]>('mcp_health_dashboard_data');
  } catch (err) {
    health.lastError = String(err);
    health.rows = [];
  } finally {
    health.loading = false;
  }
}

export async function pingServer(id: string): Promise<HealthRow | null> {
  try {
    return await invoke<HealthRow | null>('mcp_health_ping', { id });
  } catch (err) {
    health.lastError = String(err);
    return null;
  }
}

export async function startServer(id: string): Promise<boolean> {
  try {
    await invoke('mcp_start_server', { id });
    await refreshDashboard();
    return true;
  } catch (err) {
    health.lastError = String(err);
    return false;
  }
}

export async function stopServer(id: string): Promise<boolean> {
  try {
    await invoke('mcp_stop_server', { id });
    await refreshDashboard();
    return true;
  } catch (err) {
    health.lastError = String(err);
    return false;
  }
}

export function startPolling(): void {
  if (health.pollTimer !== null) {
    return;
  }
  refreshDashboard().catch(() => {
    /* swallowed — surfaced via lastError */
  });
  health.pollTimer = setInterval(() => {
    refreshDashboard().catch(() => {
      /* swallowed — surfaced via lastError */
    });
  }, health.pollIntervalMs);
}

export function stopPolling(): void {
  if (health.pollTimer !== null) {
    clearInterval(health.pollTimer);
    health.pollTimer = null;
  }
}

export async function refreshLedgerStatus(): Promise<void> {
  try {
    const raw = await invoke<{ Ok?: { rows: number }; Tampered?: { broken_seq: number; reason: string }; empty?: null } | string>('mcp_ledger_verify_chain');
    if (typeof raw === 'string') {
      // Empty variant comes through as the bare string "empty"
      health.ledgerStatus = { status: 'empty' };
      return;
    }
    if ('ok' in raw) {
      health.ledgerStatus = { status: 'ok', rows: (raw as { ok: { rows: number } }).ok.rows };
      return;
    }
    if ('tampered' in raw) {
      const t = (raw as { tampered: { broken_seq: number; reason: string } }).tampered;
      health.ledgerStatus = {
        status: 'tampered',
        broken_seq: t.broken_seq,
        reason: t.reason,
      };
      return;
    }
    health.ledgerStatus = { status: 'empty' };
  } catch (err) {
    health.lastError = String(err);
    health.ledgerStatus = null;
  }
}

export async function refreshLedgerRows(serverId: string | null, limit = 100): Promise<void> {
  try {
    health.ledgerRows = await invoke<LedgerEntry[]>('mcp_ledger_query', {
      serverId,
      limit,
      offset: 0,
    });
  } catch (err) {
    health.lastError = String(err);
    health.ledgerRows = [];
  }
}
