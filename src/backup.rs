//! Paired rollback-mode read snapshots, copied with SQLite's backup API.
//! No raw source-file opens/closes while SQLite holds POSIX inode locks.
use crate::{Result, dsn, err, integrity, now, schema, storage, store::Store};
use rusqlite::{
    Connection, OpenFlags,
    backup::{Backup, StepResult},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
    time::Duration,
};

const CORE: &str = "mail.sqlite3";
const CACHE: &str = "imap-dsn.sqlite3";
const MANIFEST: &str = "backup.json";
const CACHE_SQL: &str = "CREATE TABLE reports(remote_sha256 TEXT PRIMARY KEY, package_id TEXT NOT NULL, attempt_id TEXT NOT NULL, raw_sha256 TEXT NOT NULL, report_id TEXT NOT NULL);
CREATE TRIGGER reports_no_update BEFORE UPDATE ON reports BEGIN SELECT RAISE(ABORT,'immutable'); END;
CREATE TRIGGER reports_no_delete BEFORE DELETE ON reports BEGIN SELECT RAISE(ABORT,'immutable'); END;";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    name: String,
    bytes: u64,
    sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u8,
    created_at: String,
    files: Vec<Entry>,
}

fn checked(path: &Path) -> Result<File> {
    #[cfg(unix)]
    let file = {
        use rustix::fs::{Mode, OFlags, open};
        File::from(
            open(
                path,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| storage())?,
        )
    };
    #[cfg(not(unix))]
    let file = {
        if fs::symlink_metadata(path)
            .map_err(|_| storage())?
            .file_type()
            .is_symlink()
        {
            return Err(storage());
        }
        File::open(path).map_err(|_| storage())?
    };
    let info = file.metadata().map_err(|_| storage())?;
    if !info.is_file() {
        return Err(storage());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if info.nlink() != 1 || info.permissions().mode() & 0o077 != 0 {
            return Err(storage());
        }
    }
    Ok(file)
}
fn readonly(path: &Path) -> Result<Connection> {
    drop(checked(path)?); // Before opening SQLite, never after taking its locks.
    let path = fs::canonicalize(path).map_err(|_| storage())?;
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| storage())?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(|_| storage())?;
    Ok(conn)
}
fn begin(conn: &Connection) -> Result<()> {
    conn.execute_batch("BEGIN").map_err(|_| storage())?;
    let mode: String = conn
        .pragma_query_value(None, "journal_mode", |r| r.get(0))
        .map_err(|_| storage())?;
    if mode != "delete" {
        return Err(err(
            "backup_mode",
            "Backup requires rollback DELETE journal mode",
            5,
        ));
    }
    // Pin a real read snapshot (a bare BEGIN alone takes no SHARED lock).
    conn.query_row("SELECT count(*) FROM sqlite_schema", [], |r| {
        r.get::<_, i64>(0)
    })
    .map_err(|_| storage())?;
    Ok(())
}
fn health(conn: &Connection) -> Result<()> {
    let result: String = conn
        .query_row("PRAGMA quick_check", [], |r| r.get(0))
        .map_err(|_| storage())?;
    if result != "ok"
        || conn
            .prepare("PRAGMA foreign_key_check")
            .map_err(|_| storage())?
            .exists([])
            .map_err(|_| storage())?
    {
        return Err(integrity());
    }
    Ok(())
}
fn cache_schema(conn: &Connection) -> Result<()> {
    let identity: (i32, i32) = (
        conn.pragma_query_value(None, "application_id", |r| r.get(0))
            .map_err(|_| storage())?,
        conn.pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(|_| storage())?,
    );
    let expected = Connection::open_in_memory().map_err(|_| storage())?;
    expected.execute_batch(CACHE_SQL).map_err(|_| storage())?;
    fn shape(conn: &Connection) -> Result<Vec<(String, String, String)>> {
        conn.prepare("SELECT type,name,sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name").map_err(|_|storage())?.query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|_|storage())?.collect::<std::result::Result<Vec<_>,_>>().map_err(|_|storage())
    }
    if identity != (1296974931, 1) || shape(conn)? != shape(&expected)? {
        return Err(err("storage_schema", "Unsupported backup cache schema", 9));
    }
    Ok(())
}
fn validate_pair(store: &Store, cache: Option<&Connection>) -> Result<()> {
    schema::validate_current(&store.conn)?;
    health(&store.conn)?;
    let mut statement = store
        .conn
        .prepare("SELECT id FROM packages ORDER BY rowid")
        .map_err(|_| storage())?;
    let ids = statement
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|_| storage())?;
    for id in ids {
        store.status(&id.map_err(|_| storage())?)?;
    }
    if let Some(cache) = cache {
        cache_schema(cache)?;
        health(cache)?;
        let mut statement = cache
            .prepare(
                "SELECT package_id,attempt_id,raw_sha256,report_id FROM reports ORDER BY rowid",
            )
            .map_err(|_| storage())?;
        let rows = statement
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })
            .map_err(|_| storage())?;
        for row in rows {
            let (p, a, d, id) = row.map_err(|_| storage())?;
            let value = dsn::inspect(store, &id)
                .map_err(|_| err("backup_binding", "Cache and core snapshot do not match", 5))?;
            let record = &value["record"];
            if record["package_id"] != p || record["attempt_id"] != a || record["sha256"] != d {
                return Err(err(
                    "backup_binding",
                    "Cache and core snapshot do not match",
                    5,
                ));
            }
        }
    }
    Ok(())
}
fn exclusive(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path).map_err(|_| storage())
}
fn digest(path: &Path, name: &str) -> Result<Entry> {
    let mut file = checked(path)?;
    let mut sha = Sha256::new();
    let mut bytes = 0;
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(|_| storage())?;
        if n == 0 {
            break;
        }
        sha.update(&buffer[..n]);
        bytes += n as u64;
    }
    Ok(Entry {
        name: name.into(),
        bytes,
        sha256: format!("{:x}", sha.finalize()),
    })
}
fn copy(conn: &Connection, output: &Path, name: &str) -> Result<Entry> {
    let path = output.join(name);
    drop(exclusive(&path)?);
    let mut destination = Connection::open_with_flags(
        fs::canonicalize(&path).map_err(|_| storage())?,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| storage())?;
    destination
        .execute_batch("PRAGMA synchronous=EXTRA")
        .map_err(|_| storage())?;
    copy_pages(conn, &mut destination, || Ok(()))?;
    destination.close().map_err(|_| storage())?;
    checked(&path)?.sync_all().map_err(|_| storage())?;
    digest(&path, name)
}
// A pinned source has a fixed finite page count. Copy in bounded steps; no
// busy/restart retry loop. The callback is internal and compiled away in normal
// copying; tests synchronize process death or competing writes after real I/O.
fn copy_pages(
    source: &Connection,
    destination: &mut Connection,
    mut progress: impl FnMut() -> Result<()>,
) -> Result<()> {
    let backup = Backup::new(source, destination).map_err(|_| storage())?;
    loop {
        match backup.step(128).map_err(|_| storage())? {
            StepResult::Done => return Ok(()),
            StepResult::More => progress()?,
            _ => return Err(err("busy", "Backup snapshot is busy", 5)),
        }
    }
}
fn summary(cache: bool) -> Value {
    json!({"backup_schema_version":1,"integrity":"verified","cache_included":cache,"configuration_included":false,"credentials_included":false,"network_used":false,"freshness":"not_established","retry_authorized":false})
}
pub(super) fn create(store: &Store, output: &Path) -> Result<Value> {
    // Core first: its SHARED rollback lock prevents a later cache row referring
    // to a core commit newer than this snapshot. Older lagging cache is valid.
    let tx = store.conn.unchecked_transaction().map_err(|_| storage())?;
    let mode: String = tx
        .pragma_query_value(None, "journal_mode", |r| r.get(0))
        .map_err(|_| storage())?;
    if mode != "delete" {
        return Err(err(
            "backup_mode",
            "Backup requires rollback DELETE journal mode",
            5,
        ));
    }
    schema::validate_current(&tx)?; // Pins core before cache is opened.
    let cache_path = store.home.join(CACHE);
    let cache = match fs::symlink_metadata(&cache_path) {
        Ok(_) => {
            let c = readonly(&cache_path)?;
            begin(&c)?;
            Some(c)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => return Err(storage()),
    };
    validate_pair(store, cache.as_ref())?;
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(output).map_err(|_| storage())?;
    let mut files = vec![copy(&tx, output, CORE)?];
    if let Some(cache) = cache.as_ref() {
        files.push(copy(cache, output, CACHE)?);
    }
    // Reopen only the destination in verification; source SQLite locks remain.
    verify_databases(output, cache.is_some())?;
    let manifest = Manifest {
        schema_version: 1,
        created_at: now(),
        files,
    };
    let raw = serde_json::to_vec(&manifest).map_err(|_| storage())?;
    let mut file = exclusive(&output.join(MANIFEST))?;
    file.write_all(&raw).map_err(|_| storage())?;
    file.sync_all().map_err(|_| storage())?;
    File::open(output)
        .map_err(|_| storage())?
        .sync_all()
        .map_err(|_| storage())?;
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    File::open(parent)
        .map_err(|_| storage())?
        .sync_all()
        .map_err(|_| storage())?;
    tx.commit().map_err(|_| storage())?;
    Ok(summary(cache.is_some()))
}
fn verify_databases(source: &Path, has_cache: bool) -> Result<()> {
    let store = Store {
        conn: readonly(&source.join(CORE))?,
        home: source.into(),
    };
    begin(&store.conn)?;
    let cache = if has_cache {
        let c = readonly(&source.join(CACHE))?;
        begin(&c)?;
        Some(c)
    } else {
        None
    };
    validate_pair(&store, cache.as_ref())
}
fn directory(source: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(source).map_err(|_| storage())?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err(storage());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            return Err(storage());
        }
    }
    Ok(())
}
pub(super) fn create_from_home(home: &Path, output: &Path) -> Result<Value> {
    directory(home)?;
    let store = Store {
        conn: readonly(&home.join(CORE))?,
        home: home.into(),
    };
    create(&store, output)
}
pub(super) fn verify(source: &Path) -> Result<Value> {
    directory(source)?;
    let mut raw = Vec::new();
    checked(&source.join(MANIFEST))?
        .take(16385)
        .read_to_end(&mut raw)
        .map_err(|_| storage())?;
    if raw.len() > 16384 {
        return Err(integrity());
    }
    let manifest: Manifest = serde_json::from_slice(&raw).map_err(|_| integrity())?;
    let names: Vec<_> = manifest.files.iter().map(|f| f.name.as_str()).collect();
    if manifest.schema_version != 1
        || !(names == [CORE] || names == [CORE, CACHE])
        || chrono::DateTime::parse_from_rfc3339(&manifest.created_at).is_err()
    {
        return Err(integrity());
    }
    let mut actual = fs::read_dir(source)
        .map_err(|_| storage())?
        .map(|e| e.map(|e| e.file_name()).map_err(|_| storage()))
        .collect::<Result<Vec<_>>>()?;
    actual.sort();
    let mut expected: Vec<_> = names
        .iter()
        .copied()
        .chain([MANIFEST])
        .map(std::ffi::OsString::from)
        .collect();
    expected.sort();
    if actual != expected {
        return Err(integrity());
    }
    for file in &manifest.files {
        let observed = digest(&source.join(&file.name), &file.name)?;
        if observed.bytes != file.bytes || observed.sha256 != file.sha256 {
            return Err(integrity());
        }
    }
    verify_databases(source, names.contains(&CACHE))?;
    Ok(summary(names.contains(&CACHE)))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // Synthetic fixtures may panic on setup failures.
mod tests {
    use super::*;
    #[test]
    fn source_rollback_lock_survives_copy_and_destination_hashing() {
        let root = tempfile::tempdir().unwrap();
        let source = Store::open(root.path().join("source")).unwrap();
        let output = root.path().join("snapshot");
        fs::create_dir(&output).unwrap();
        let tx = source.conn.unchecked_transaction().unwrap();
        schema::validate_current(&tx).unwrap();
        copy(&tx, &output, CORE).unwrap();
        // Regression for accidentally opening/closing the source inode to hash
        // raw bytes: POSIX close() would silently cancel SQLite's file locks.
        let check = || {
            // A second SQLite connection in this process is insufficient: its
            // internal lock bookkeeping can conceal POSIX close() lock loss.
            let script = "import sqlite3,sys\nc=sqlite3.connect(sys.argv[1],timeout=0)\ntry:\n c.executescript('BEGIN IMMEDIATE; PRAGMA user_version=6; COMMIT;')\n print('committed')\nexcept sqlite3.OperationalError as e:\n assert 'locked' in str(e)\n print('blocked')\nfinally:\n c.close()";
            let result = std::process::Command::new("python3")
                .args(["-B", "-c", script])
                .arg(source.home.join(CORE))
                .output()
                .unwrap();
            assert!(result.status.success());
            assert!(result.stderr.is_empty());
            String::from_utf8(result.stdout).unwrap()
        };
        assert_eq!(check(), "blocked\n");
        tx.commit().unwrap();
        assert_eq!(check(), "committed\n");
    }
    #[test]
    fn wal_cache_is_refused_without_publishing_a_backup() {
        let root = tempfile::tempdir().unwrap();
        let source = Store::open(root.path().join("source")).unwrap();
        let cache = source.home.join(CACHE);
        drop(exclusive(&cache).unwrap());
        let conn = Connection::open(&cache).unwrap();
        conn.execute_batch(CACHE_SQL).unwrap();
        conn.execute_batch(
            "PRAGMA application_id=1296974931; PRAGMA user_version=1; PRAGMA journal_mode=WAL;",
        )
        .unwrap();
        conn.close().unwrap();
        let before = fs::read(&cache).unwrap();
        let output = root.path().join("snapshot");
        let error = create(&source, &output).unwrap_err();
        assert_eq!(error.code, "backup_mode");
        assert!(!output.exists());
        assert_eq!(fs::read(&cache).unwrap(), before);
    }
    fn fault_source(root: &Path) -> Store {
        use clap::Parser;
        fs::write(root.join("config.toml"), "sender='sender@example.org'\n").unwrap();
        fs::write(root.join("subject"), "Synthetic backup fault").unwrap();
        fs::write(root.join("body"), "Synthetic letter").unwrap();
        let mut pdf = vec![b'x'; 2 * 1024 * 1024];
        pdf[..5].copy_from_slice(b"%PDF-");
        fs::write(root.join("resume.pdf"), pdf).unwrap();
        let args = vec![
            "agent-mail".into(),
            "--home".into(),
            root.join("source").into_os_string(),
            "--config".into(),
            root.join("config.toml").into_os_string(),
            "prepare".into(),
            "--to".into(),
            "recipient@example.org".into(),
            "--subject-file".into(),
            root.join("subject").into_os_string(),
            "--body-file".into(),
            root.join("body").into_os_string(),
            "--attach".into(),
            root.join("resume.pdf").into_os_string(),
        ];
        crate::run(crate::Cli::try_parse_from(args).unwrap()).unwrap();
        Store::open(root.join("source")).unwrap()
    }
    #[test]
    fn destination_page_budget_causes_sqlite_full_and_preserves_source() {
        let root = tempfile::tempdir().unwrap();
        let source = fault_source(root.path());
        let before = fs::read(source.home.join(CORE)).unwrap(); // Before source locks.
        let tx = source.conn.unchecked_transaction().unwrap();
        schema::validate_current(&tx).unwrap();
        let folder = root.path().join("partial");
        fs::create_dir(&folder).unwrap();
        drop(exclusive(&folder.join(CORE)).unwrap());
        let mut target = Connection::open(folder.join(CORE)).unwrap();
        target.execute_batch("PRAGMA max_page_count=8").unwrap();
        assert_eq!(copy_pages(&tx, &mut target, || Ok(())).unwrap_err().exit, 7);
        let backup = Backup::new(&tx, &mut target).unwrap();
        let failure = backup.step(128).unwrap_err();
        assert!(
            matches!(failure,rusqlite::Error::SqliteFailure(error,_) if error.code==rusqlite::ErrorCode::DiskFull)
        );
        drop(backup);
        target.close().unwrap();
        tx.commit().unwrap();
        assert_eq!(fs::read(source.home.join(CORE)).unwrap(), before);
        assert!(!folder.join(MANIFEST).exists());
    }
    #[test]
    fn mid_copy_child_fixture() {
        let Some(root) = std::env::var_os("AGENT_MAIL_UNIT_BACKUP_CHILD") else {
            return;
        };
        let root = std::path::PathBuf::from(root);
        let source = readonly(&root.join("source").join(CORE)).unwrap();
        begin(&source).unwrap();
        let folder = root.join("partial");
        fs::create_dir(&folder).unwrap();
        drop(exclusive(&folder.join(CORE)).unwrap());
        let mut target = Connection::open(folder.join(CORE)).unwrap();
        copy_pages(&source, &mut target, || {
            fs::write(root.join("copy-started"), b"synthetic ready").unwrap();
            loop {
                std::thread::park();
            }
        })
        .unwrap();
        panic!("Fixture must block after a real partial backup step");
    }
    #[test]
    fn killed_mid_copy_refuses_partial_and_releases_writer_lock() {
        use std::process::{Command, Stdio};
        let root = tempfile::tempdir().unwrap();
        let source = fault_source(root.path());
        let before = fs::read(source.home.join(CORE)).unwrap();
        struct Child(std::process::Child);
        impl Drop for Child {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let mut child = Child(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "backup::tests::mid_copy_child_fixture",
                    "--nocapture",
                ])
                .env("AGENT_MAIL_UNIT_BACKUP_CHILD", root.path())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let until = std::time::Instant::now() + Duration::from_secs(10);
        while !root.path().join("copy-started").exists() {
            assert!(
                std::time::Instant::now() < until,
                "Child did not reach the partial-copy barrier"
            );
            assert!(child.0.try_wait().unwrap().is_none());
            std::thread::sleep(Duration::from_millis(10));
        }
        let writer = || {
            let result=Command::new("python3").args(["-B","-c","import sqlite3,sys\nc=sqlite3.connect(sys.argv[1],timeout=0)\ntry:\n c.execute(\"INSERT INTO attempts SELECT 'synthetic-later',id,'2026-01-01T00:00:00+00:00',NULL,'unknown',json_extract(CAST(manifest AS TEXT),'$.message_id'),NULL FROM packages LIMIT 1\")\n c.commit()\n print('committed')\nexcept sqlite3.OperationalError as e:\n assert 'locked' in str(e)\n print('blocked')\nfinally:\n c.close()"]).arg(source.home.join(CORE)).output().unwrap();
            assert!(result.status.success());
            assert!(result.stderr.is_empty());
            String::from_utf8(result.stdout).unwrap()
        };
        assert_eq!(writer(), "blocked\n");
        child.0.kill().unwrap();
        assert!(!child.0.wait().unwrap().success());
        assert_eq!(fs::read(source.home.join(CORE)).unwrap(), before);
        assert!(verify(&root.path().join("partial")).is_err());
        assert_eq!(writer(), "committed\n");
        let output = root.path().join("complete");
        create(&source, &output).unwrap();
        verify(&output).unwrap();
        let id: String = source
            .conn
            .query_row("SELECT id FROM packages", [], |r| r.get(0))
            .unwrap();
        assert_eq!(source.status(&id).unwrap()["state"], "unknown");
        let restored = Store {
            conn: readonly(&output.join(CORE)).unwrap(),
            home: output,
        };
        assert_eq!(restored.status(&id).unwrap()["state"], "unknown");
    }
}
