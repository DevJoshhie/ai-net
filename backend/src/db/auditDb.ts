/**
 * Audit database pool — issue #655.
 *
 * Manages the singleton SQLite pool for audit.db, applying migrations on
 * first open.
 */

import path from "path";
import { createPool, type SqlitePool } from "./pool";
import { migrateToLatest } from "./migrator";
import { poolSettings } from "./poolConfig";

const MIGRATIONS_DIR = path.join(__dirname, "migrations", "audit");

let _pool: SqlitePool | null = null;

export function getAuditPool(dbPath?: string): SqlitePool {
  if (_pool && !_pool.closed) return _pool;
  const filePath = dbPath ?? process.env.AUDIT_DB_PATH ?? path.join(process.cwd(), "audit.db");
  _pool = createPool({
    filePath,
    ...poolSettings(),
    onCreate(db) {
      migrateToLatest(db, MIGRATIONS_DIR);
    },
  });
  return _pool;
}

export async function closeAuditDb(): Promise<void> {
  if (_pool && !_pool.closed) {
    await _pool.close();
    _pool = null;
  }
}
