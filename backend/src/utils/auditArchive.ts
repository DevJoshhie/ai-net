#!/usr/bin/env node
/**
 * audit:archive — export audit_log rows older than 90 days to a JSON file
 * and optionally delete them from the database.
 *
 * Usage: node dist/utils/auditArchive.js [--delete]
 * Or via npm: npm run audit:archive [-- --delete]
 */

import fs from "fs";
import path from "path";
import { createPool } from "../db/pool";
import { migrateToLatest } from "../db/migrator";
import { poolSettings } from "../db/poolConfig";

const MIGRATIONS_DIR = path.join(__dirname, "..", "db", "migrations", "audit");
const DELETE_AFTER_ARCHIVE =
  process.argv.includes("--delete") || process.env.AUDIT_ARCHIVE_DELETE === "true";

const RETENTION_DAYS = parseInt(process.env.AUDIT_RETENTION_DAYS ?? "90", 10);

async function main(): Promise<void> {
  const filePath = process.env.AUDIT_DB_PATH ?? path.join(process.cwd(), "audit.db");
  const pool = createPool({
    filePath,
    ...poolSettings(),
    onCreate(db) {
      migrateToLatest(db, MIGRATIONS_DIR);
    },
  });

  const cutoff = new Date();
  cutoff.setDate(cutoff.getDate() - RETENTION_DAYS);
  const cutoffIso = cutoff.toISOString();

  const rows = await pool.read((db) =>
    db.prepare("SELECT * FROM audit_log WHERE timestamp < ? ORDER BY id ASC").all(cutoffIso),
  );

  if (rows.length === 0) {
    console.log(`No audit_log rows older than ${RETENTION_DAYS} days found.`);
    await pool.close();
    return;
  }

  const archiveDir =
    process.env.AUDIT_ARCHIVE_DIR ?? path.join(process.cwd(), "audit-archives");
  fs.mkdirSync(archiveDir, { recursive: true });

  const filename = `audit_archive_${new Date().toISOString().replace(/[:.]/g, "-")}.json`;
  const filepath = path.join(archiveDir, filename);
  fs.writeFileSync(filepath, JSON.stringify(rows, null, 2), "utf8");
  console.log(`Archived ${rows.length} rows to ${filepath}`);

  if (DELETE_AFTER_ARCHIVE) {
    await pool.write((db) => {
      db.prepare("DELETE FROM audit_log WHERE timestamp < ?").run(cutoffIso);
    });
    console.log(`Deleted ${rows.length} archived rows from audit_log.`);
  }

  await pool.close();
}

main().catch((err) => {
  console.error("audit:archive failed:", err);
  process.exit(1);
});
