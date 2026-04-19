// SPDX-License-Identifier: MIT
// Sandbox store — Svelte 5 runes.  Wires CodeCell / CellOutput /
// SandboxControls / ResourceMonitor to the Rust code_sandbox_lite
// backend via Tauri invoke.
//
// Cybercrime liability defence — see src-tauri/code_sandbox_lite/mod.rs.

import { invoke } from '@tauri-apps/api/core';

// ─── Type mirrors of the Rust types ─────────────────────────────────────

export type Lang = 'python' | 'javascript' | 'wasm';

export type TrustLevel = 'low' | 'medium' | 'high';

export type CellStatus = 'pending' | 'running' | 'succeeded' | 'failed' | 'cancelled' | 'limit_exceeded';

export interface Cell {
  id: string;
  session_id: string;
  lang: Lang;
  source: string;
  created_unix: number;
  trust_level: TrustLevel;
  preopen: string | null;
  env: [string, string][];
  source_sha256: string;
  tags: string[];
}

export interface Limits {
  memory_mib: number;
  time_secs: number;
  stdout_cap: number;
  stderr_cap: number;
  disk_mib: number;
}

export const DEFAULT_LIMITS: Limits = {
  memory_mib: 512,
  time_secs: 60,
  stdout_cap: 16 * 1024 * 1024,
  stderr_cap: 16 * 1024 * 1024,
  disk_mib: 64,
};

export interface ResourceUsage {
  memory_peak_bytes: number;
  fuel_consumed: number;
  wall_time_ms: number;
  cpu_time_ms: number;
  disk_bytes_written: number;
  stdout_bytes: number;
  stderr_bytes: number;
}

export type Output =
  | { kind: 'text'; stream: 'stdout' | 'stderr'; data: string; ts_unix: number }
  | { kind: 'image'; mime: string; base64: string; ts_unix: number }
  | { kind: 'html'; data: string; ts_unix: number }
  | { kind: 'table'; headers: string[]; rows: unknown[][]; ts_unix: number }
  | { kind: 'file'; path: string; mime: string; bytes: number; ts_unix: number }
  | { kind: 'error'; ename: string; evalue: string; traceback: string[]; ts_unix: number }
  | { kind: 'result'; repr: string; ts_unix: number };

export interface CellResult {
  cell_id: string;
  status: CellStatus;
  exit_code: number;
  outputs: Output[];
  usage: ResourceUsage;
  started_unix: number;
  finished_unix: number;
  error: string | null;
  audit_entry_id: string | null;
}

export interface BudgetSnapshot {
  session_id_hash: number;
  memory_used_bytes: number;
  memory_peak_bytes: number;
  fuel_consumed: number;
  wall_time_ms_total: number;
  disk_bytes_written: number;
  cells_run: number;
  cells_succeeded: number;
  cells_failed: number;
  cells_limit_exceeded: number;
}

export interface Variable {
  name: string;
  type_name: string;
  repr: string;
  size_bytes: number;
  value_json: unknown;
}

export interface MountSpec {
  host_path: string;
  guest_path: string;
  read_only: boolean;
}

export interface AuditEntry {
  id: string;
  cell_id: string;
  session_id: string;
  source_sha256: string;
  output_sha256: string;
  status: CellStatus;
  exit_code: number;
  fuel_consumed: number;
  wall_time_ms: number;
  timestamp_unix: number;
  trust_level: TrustLevel;
  prev_hash: string;
  self_hash: string;
}

export interface ProvenanceEdge {
  cell_id: string;
  session_id: string;
  input_ids: string[];
  output_sha256: string;
  model_name: string | null;
  timestamp_unix: number;
  trust_level: TrustLevel;
}

// ─── Reactive store ─────────────────────────────────────────────────────

class SandboxStore {
  sessionId = $state<string>(`sess-${Math.random().toString(36).slice(2, 10)}`);
  cells = $state<Cell[]>([]);
  results = $state<Map<string, CellResult>>(new Map());
  limits = $state<Limits>({ ...DEFAULT_LIMITS });
  budget = $state<BudgetSnapshot | null>(null);
  variables = $state<Variable[]>([]);
  mounts = $state<MountSpec[]>([]);
  audit = $state<AuditEntry[]>([]);
  pyodideCached = $state<boolean>(false);
  busy = $state<boolean>(false);
  lastError = $state<string | null>(null);

  async createCell(opts: {
    lang: Lang;
    source: string;
    trustLevel?: TrustLevel;
    preopen?: string;
    env?: [string, string][];
    tags?: string[];
  }): Promise<Cell> {
    this.busy = true;
    this.lastError = null;
    try {
      const cell = await invoke<Cell>('sandbox_create_cell', {
        sessionId: this.sessionId,
        lang: opts.lang,
        source: opts.source,
        trustLevel: opts.trustLevel ?? null,
        preopen: opts.preopen ?? null,
        env: opts.env ?? null,
        tags: opts.tags ?? null,
      });
      this.cells = [...this.cells, cell];
      return cell;
    } catch (e) {
      this.lastError = String(e);
      throw e;
    } finally {
      this.busy = false;
    }
  }

  async runCell(cellId: string): Promise<CellResult> {
    this.busy = true;
    this.lastError = null;
    try {
      const result = await invoke<CellResult>('sandbox_run_cell', { cellId });
      const next = new Map(this.results);
      next.set(cellId, result);
      this.results = next;
      await this.refreshBudget();
      await this.refreshVariables();
      return result;
    } catch (e) {
      this.lastError = String(e);
      throw e;
    } finally {
      this.busy = false;
    }
  }

  async stopCell(cellId: string): Promise<boolean> {
    return invoke<boolean>('sandbox_stop_cell', { cellId });
  }

  async clearCell(cellId: string): Promise<void> {
    await invoke('sandbox_clear_cell', { cellId });
    const next = new Map(this.results);
    next.delete(cellId);
    this.results = next;
  }

  async refreshBudget(): Promise<void> {
    this.budget = await invoke<BudgetSnapshot>('sandbox_resource_status', {
      sessionId: this.sessionId,
    });
  }

  async refreshVariables(): Promise<void> {
    this.variables = await invoke<Variable[]>('sandbox_get_variables', {
      sessionId: this.sessionId,
    });
  }

  async refreshMounts(): Promise<void> {
    this.mounts = await invoke<MountSpec[]>('sandbox_list_attached_files', {
      sessionId: this.sessionId,
    });
  }

  async refreshAudit(): Promise<void> {
    this.audit = await invoke<AuditEntry[]>('sandbox_audit_log', { limit: 64 });
  }

  async setLimits(limits: Limits): Promise<void> {
    await invoke('sandbox_set_limits', { sessionId: this.sessionId, limits });
    this.limits = { ...limits };
  }

  async healthCheck(): Promise<unknown> {
    const h = await invoke<{ pyodide_cached: boolean }>('sandbox_health_check');
    this.pyodideCached = h.pyodide_cached;
    return h;
  }

  async attachFile(hostPath: string, guestPath: string, readOnly: boolean): Promise<void> {
    await invoke('sandbox_attach_file', {
      sessionId: this.sessionId,
      hostPath,
      guestPath,
      readOnly,
    });
    await this.refreshMounts();
  }

  async detachFile(guestPath: string): Promise<void> {
    await invoke('sandbox_detach_file', {
      sessionId: this.sessionId,
      guestPath,
    });
    await this.refreshMounts();
  }

  async exportNotebook(): Promise<string> {
    return invoke<string>('sandbox_export_notebook', { sessionId: this.sessionId });
  }

  async importNotebook(json: string): Promise<string> {
    const newId = await invoke<string>('sandbox_import_notebook', { json });
    this.sessionId = newId;
    return newId;
  }

  async quickPython(source: string): Promise<CellResult> {
    return invoke<CellResult>('sandbox_quick_python', { source });
  }

  async quickJs(source: string): Promise<CellResult> {
    return invoke<CellResult>('sandbox_quick_js', { source });
  }
}

export const sandbox = new SandboxStore();
