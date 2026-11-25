/**
 * Audit logging utility — issue #655.
 *
 * Provides logAuditEvent() which inserts an append-only row into the
 * audit_log table. Sensitive fields are stripped before the payload is
 * persisted so private keys, tokens, and passwords never reach the database.
 */

import path from "path";
import { createPool, type SqlitePool } from "../db/pool";
import { migrateToLatest } from "../db/migrator";
import { poolSettings } from "../db/poolConfig";
import { createLogger } from "./logger";

const logger = createLogger({ component: "audit-log" });

// Fields that must never be stored in the audit log payload.
const SENSITIVE_KEYS = new Set([
  "privateKey",
  "private_key",
  "secret",
  "password",
  "token",
  "Authorization",
  "authorization",
  "apiKey",
  "api_key",
]);

const MIGRATIONS_DIR = path.join(__dirname, "..", "db", "migrations", "audit");

let _pool: SqlitePool | null = null;

function getPool(): SqlitePool {
  if (_pool && !_pool.closed) return _pool;
  const filePath = process.env.AUDIT_DB_PATH ?? path.join(process.cwd(), "audit.db");
  _pool = createPool({
    filePath,
    ...poolSettings(),
    onCreate(db) {
      migrateToLatest(db, MIGRATIONS_DIR);
    },
  });
  return _pool;
}

export function closeAuditDb(): Promise<void> {
  return _pool?.close() ?? Promise.resolve();
}

/**
 * Recursively removes sensitive keys from an object, returning a new object.
 * Arrays are traversed; non-object values are returned as-is.
 */
export function scrubSensitiveFields(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(scrubSensitiveFields);
  }
  if (value !== null && typeof value === "object") {
    const result: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(value as Record<string, unknown>)) {
      if (SENSITIVE_KEYS.has(k)) continue;
      result[k] = scrubSensitiveFields(v);
    }
    return result;
  }
  return value;
}

export interface AuditEventInput {
  action: string;
  actorWallet?: string | null;
  entityType: string;
  entityId: string;
  payload: unknown;
  ipAddress?: string | null;
  userAgent?: string | null;
}

/**
 * Insert one audit event row. Strips sensitive payload fields before writing.
 * Errors are caught and logged — a failed audit write must never abort the
 * primary request.
 */
export async function logAuditEvent(input: AuditEventInput): Promise<void> {
  try {
    const pool = getPool();
    const scrubbed = scrubSensitiveFields(input.payload);
    const payloadJson = JSON.stringify(scrubbed);
    const timestamp = new Date().toISOString();

    await pool.write((db) => {
      db.prepare(
        `INSERT INTO audit_log
           (timestamp, action, actorWallet, entityType, entityId, payload, ipAddress, userAgent)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
      ).run(
        timestamp,
        input.action,
        input.actorWallet ?? null,
        input.entityType,
        input.entityId,
        payloadJson,
        input.ipAddress ?? null,
        input.userAgent ?? null,
      );
    });
  } catch (err) {
    logger.error({ err }, "failed to write audit log entry");
  }
}
