CREATE TABLE IF NOT EXISTS audit_log (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  timestamp   TEXT NOT NULL,
  action      TEXT NOT NULL,
  actorWallet TEXT,
  entityType  TEXT NOT NULL,
  entityId    TEXT NOT NULL,
  payload     TEXT NOT NULL,
  ipAddress   TEXT,
  userAgent   TEXT
);

-- Prevent any UPDATE on audit_log
CREATE TRIGGER IF NOT EXISTS audit_log_no_update
BEFORE UPDATE ON audit_log
BEGIN
  SELECT RAISE(FAIL, 'audit_log is append-only: UPDATE is not permitted');
END;

-- Prevent any DELETE on audit_log
CREATE TRIGGER IF NOT EXISTS audit_log_no_delete
BEFORE DELETE ON audit_log
BEGIN
  SELECT RAISE(FAIL, 'audit_log is append-only: DELETE is not permitted');
END;
