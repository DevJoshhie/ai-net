/**
 * GET /api/audit — query endpoint for the audit log — issue #655.
 *
 * Query params: entityId, entityType, action, actorWallet,
 *               from (ISO date), to (ISO date), page, limit (max 100).
 * Returns: { data, total, page, totalPages }
 */

import { Router, type Request, type Response } from "express";
import path from "path";
import { createPool, type SqlitePool } from "../../db/pool";
import { migrateToLatest } from "../../db/migrator";
import { poolSettings } from "../../db/poolConfig";

const MIGRATIONS_DIR = path.join(__dirname, "..", "..", "db", "migrations", "audit");

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

export interface AuditRow {
  id: number;
  timestamp: string;
  action: string;
  actorWallet: string | null;
  entityType: string;
  entityId: string;
  payload: string;
  ipAddress: string | null;
  userAgent: string | null;
}

export function createAuditRouter(): Router {
  const router = Router();

  /**
   * @openapi
   * /api/audit:
   *   get:
   *     summary: Query the audit log
   *     operationId: queryAuditLog
   *     tags: [Audit]
   *     parameters:
   *       - in: query
   *         name: entityId
   *         schema: { type: string }
   *       - in: query
   *         name: entityType
   *         schema: { type: string }
   *       - in: query
   *         name: action
   *         schema: { type: string }
   *       - in: query
   *         name: actorWallet
   *         schema: { type: string }
   *       - in: query
   *         name: from
   *         schema: { type: string, format: date }
   *       - in: query
   *         name: to
   *         schema: { type: string, format: date }
   *       - in: query
   *         name: page
   *         schema: { type: integer, default: 1 }
   *       - in: query
   *         name: limit
   *         schema: { type: integer, default: 20, maximum: 100 }
   *     responses:
   *       200:
   *         description: Paginated audit log results
   */
  router.get("/", async (req: Request, res: Response) => {
    try {
      const {
        entityId,
        entityType,
        action,
        actorWallet,
        from,
        to,
      } = req.query as Record<string, string | undefined>;

      const page = Math.max(1, parseInt((req.query.page as string) ?? "1", 10) || 1);
      const limit = Math.min(
        100,
        Math.max(1, parseInt((req.query.limit as string) ?? "20", 10) || 20),
      );
      const offset = (page - 1) * limit;

      const conditions: string[] = [];
      const params: unknown[] = [];

      if (entityId) { conditions.push("entityId = ?"); params.push(entityId); }
      if (entityType) { conditions.push("entityType = ?"); params.push(entityType); }
      if (action) { conditions.push("action = ?"); params.push(action); }
      if (actorWallet) { conditions.push("actorWallet = ?"); params.push(actorWallet); }
      if (from) { conditions.push("timestamp >= ?"); params.push(from); }
      if (to) { conditions.push("timestamp <= ?"); params.push(to + "T23:59:59.999Z"); }

      const where = conditions.length > 0 ? `WHERE ${conditions.join(" AND ")}` : "";

      const pool = getPool();

      const [total, rows] = await pool.read((db) => {
        const countRow = db
          .prepare(`SELECT COUNT(*) as cnt FROM audit_log ${where}`)
          .get(...params) as { cnt: number };

        const dataRows = db
          .prepare(
            `SELECT id, timestamp, action, actorWallet, entityType, entityId,
                    payload, ipAddress, userAgent
             FROM audit_log ${where}
             ORDER BY id DESC
             LIMIT ? OFFSET ?`,
          )
          .all(...params, limit, offset) as AuditRow[];

        return [countRow.cnt, dataRows];
      });

      const totalPages = Math.ceil(total / limit);

      res.json({
        data: rows.map((r) => ({
          ...r,
          payload: (() => {
            try {
              return JSON.parse(r.payload) as unknown;
            } catch {
              return r.payload;
            }
          })(),
        })),
        total,
        page,
        totalPages,
      });
    } catch (err) {
      res.status(500).json({ error: "Failed to query audit log", message: String(err) });
    }
  });

  return router;
}
