/**
 * Transport abstraction — the single seam that lets the same frontend run on
 * the Tauri desktop (IPC + SQL plugin) or in a browser (HTTP to velo-server).
 *
 * Everything that previously called `invoke()` or the Tauri SQL plugin goes
 * through a `Transport`. The active transport is chosen once at startup:
 *   - `tauriTransport` when running inside the Tauri webview
 *   - `httpTransport`  when running as a plain web page (served by velo-server)
 */

export interface ExecuteResult {
  /** Number of rows affected by an INSERT/UPDATE/DELETE. */
  rowsAffected: number;
  /** Row id of the last INSERT, when applicable. */
  lastInsertId?: number;
}

export interface Transport {
  /** Invoke a backend command (the web side maps these to HTTP routes). */
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
  /** Run a SELECT and return typed rows. */
  select<T>(query: string, params?: unknown[]): Promise<T>;
  /** Run an INSERT/UPDATE/DELETE/DDL statement. */
  execute(query: string, params?: unknown[]): Promise<ExecuteResult>;
}
