CREATE TABLE IF NOT EXISTS packages(id TEXT PRIMARY KEY, manifest BLOB NOT NULL, digest TEXT NOT NULL, eml BLOB NOT NULL);
CREATE TABLE IF NOT EXISTS attachments(package_id TEXT NOT NULL REFERENCES packages(id), idx INTEGER NOT NULL, content BLOB NOT NULL, PRIMARY KEY(package_id,idx));
CREATE TABLE IF NOT EXISTS attempts(id TEXT PRIMARY KEY, package_id TEXT NOT NULL REFERENCES packages(id), started_at TEXT NOT NULL, finished_at TEXT, outcome TEXT NOT NULL CHECK(outcome IN ('unknown','accepted','rejected')), message_id TEXT NOT NULL, smtp_code TEXT);
CREATE UNIQUE INDEX IF NOT EXISTS one_unresolved_submission ON attempts(package_id) WHERE outcome IN ('unknown','accepted');
CREATE TRIGGER IF NOT EXISTS packages_no_update BEFORE UPDATE ON packages BEGIN SELECT RAISE(ABORT,'immutable'); END;
CREATE TRIGGER IF NOT EXISTS packages_no_delete BEFORE DELETE ON packages BEGIN SELECT RAISE(ABORT,'immutable'); END;
CREATE TRIGGER IF NOT EXISTS attachments_no_update BEFORE UPDATE ON attachments BEGIN SELECT RAISE(ABORT,'immutable'); END;
CREATE TRIGGER IF NOT EXISTS attachments_no_delete BEFORE DELETE ON attachments BEGIN SELECT RAISE(ABORT,'immutable'); END;
