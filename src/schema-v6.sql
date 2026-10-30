CREATE TABLE dsn_header_reports(id TEXT PRIMARY KEY, package_id TEXT NOT NULL REFERENCES packages(id), attempt_id TEXT NOT NULL REFERENCES attempts(id), recorded_at TEXT NOT NULL, raw BLOB NOT NULL, digest TEXT NOT NULL, bytes INTEGER NOT NULL, UNIQUE(attempt_id,digest));
CREATE TRIGGER dsn_header_reports_no_update BEFORE UPDATE ON dsn_header_reports BEGIN SELECT RAISE(ABORT,'immutable'); END;
CREATE TRIGGER dsn_header_reports_no_delete BEFORE DELETE ON dsn_header_reports BEGIN SELECT RAISE(ABORT,'immutable'); END;
CREATE TRIGGER dsn_header_reports_binding BEFORE INSERT ON dsn_header_reports
WHEN NOT EXISTS(SELECT 1 FROM attempts WHERE id=NEW.attempt_id AND package_id=NEW.package_id AND outcome IN ('unknown','accepted'))
BEGIN SELECT RAISE(ABORT,'invalid DSN attempt'); END;
