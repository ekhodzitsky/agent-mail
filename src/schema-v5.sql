CREATE TABLE prepare_operations(key_sha256 TEXT PRIMARY KEY, input_sha256 TEXT NOT NULL, package_id TEXT NOT NULL UNIQUE REFERENCES packages(id), prepared_at TEXT NOT NULL);
CREATE TRIGGER prepare_operations_no_update BEFORE UPDATE ON prepare_operations BEGIN SELECT RAISE(ABORT,'immutable'); END;
CREATE TRIGGER prepare_operations_no_delete BEFORE DELETE ON prepare_operations BEGIN SELECT RAISE(ABORT,'immutable'); END;
