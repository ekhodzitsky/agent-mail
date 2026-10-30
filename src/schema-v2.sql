CREATE TABLE reconciliations(id TEXT PRIMARY KEY, package_id TEXT NOT NULL REFERENCES packages(id), attempt_id TEXT NOT NULL REFERENCES attempts(id), recorded_at TEXT NOT NULL, evidence BLOB NOT NULL, digest TEXT NOT NULL, bytes INTEGER NOT NULL, UNIQUE(attempt_id,digest));
CREATE TRIGGER reconciliations_no_update BEFORE UPDATE ON reconciliations BEGIN SELECT RAISE(ABORT,'immutable'); END;
CREATE TRIGGER reconciliations_no_delete BEFORE DELETE ON reconciliations BEGIN SELECT RAISE(ABORT,'immutable'); END;
CREATE TRIGGER reconciliations_unknown_only BEFORE INSERT ON reconciliations
WHEN NOT EXISTS(SELECT 1 FROM attempts WHERE id=NEW.attempt_id AND package_id=NEW.package_id AND outcome='unknown')
BEGIN SELECT RAISE(ABORT,'not an unknown attempt'); END;
