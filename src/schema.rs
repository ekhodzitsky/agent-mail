//! Recognize exact supported layouts before any schema change. Migrations never
//! rewrite package or attempt data and commit their identity atomically.
use super::{Result, err, storage};
use rusqlite::Connection;
use rusqlite::TransactionBehavior;

pub(super) const VERSION: i32 = 6;
const APPLICATION_ID: i32 = 0x414d4c52; // AMLR
const V0: &str = include_str!("schema-v0.sql");
const V1: &str = "
CREATE TRIGGER attempts_no_delete BEFORE DELETE ON attempts BEGIN SELECT RAISE(ABORT,'immutable'); END;
CREATE TRIGGER attempts_final_once BEFORE UPDATE ON attempts
WHEN OLD.outcome != 'unknown' OR OLD.finished_at IS NOT NULL
  OR NEW.id IS NOT OLD.id OR NEW.package_id IS NOT OLD.package_id
  OR NEW.started_at IS NOT OLD.started_at OR NEW.message_id IS NOT OLD.message_id
  OR NEW.finished_at IS NULL
BEGIN SELECT RAISE(ABORT,'immutable'); END;";
const V2: &str = include_str!("schema-v2.sql");
const V3: &str = include_str!("schema-v3.sql");
const V4: &str = include_str!("schema-v4.sql");
const V5: &str = include_str!("schema-v5.sql");
const V6: &str = include_str!("schema-v6.sql");
fn unsupported() -> super::Error {
    err(
        "storage_schema",
        "Unsupported or altered database schema; no migration applied",
        9,
    )
}
fn identity(conn: &Connection) -> Result<(i32, i32)> {
    let version = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(|_| storage())?;
    let app = conn
        .pragma_query_value(None, "application_id", |r| r.get(0))
        .map_err(|_| storage())?;
    Ok((version, app))
}
fn layout(conn: &Connection) -> Result<Vec<(String, String, String)>> {
    let mut stmt = conn.prepare("SELECT type,name,sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name").map_err(|_| storage())?;
    let rows = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, String>(2)?)))
        .map_err(|_| storage())?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| storage())
}
fn expected(version: i32) -> Result<Vec<(String, String, String)>> {
    let conn = Connection::open_in_memory().map_err(|_| storage())?;
    conn.execute_batch(V0).map_err(|_| storage())?;
    if version >= 1 {
        conn.execute_batch(V1).map_err(|_| storage())?;
    }
    if version >= 2 {
        conn.execute_batch(V2).map_err(|_| storage())?;
    }
    if version >= 3 {
        conn.execute_batch(V3).map_err(|_| storage())?;
    }
    if version >= 4 {
        conn.execute_batch(V4).map_err(|_| storage())?;
    }
    if version >= 5 {
        conn.execute_batch(V5).map_err(|_| storage())?;
    }
    if version >= 6 {
        conn.execute_batch(V6).map_err(|_| storage())?;
    }
    layout(&conn)
}
fn validate(conn: &Connection) -> Result<bool> {
    let (version, app) = identity(conn)?;
    if !matches!((version, app), (0, 0) | (1..=VERSION, APPLICATION_ID)) {
        return Err(unsupported());
    }
    let actual = layout(conn)?;
    let empty = actual.is_empty() && version == 0;
    if !empty && actual != expected(version)? {
        return Err(unsupported());
    }
    Ok(empty)
}
pub(super) fn initialize(conn: &mut Connection) -> Result<()> {
    // Fail closed on foreign/future/partial layouts before journal/schema writes.
    let read = conn.transaction().map_err(|_| storage())?;
    validate(&read)?;
    read.commit().map_err(|_| storage())?;
    conn.execute_batch(
        "PRAGMA foreign_keys=ON; PRAGMA journal_mode=DELETE; PRAGMA synchronous=EXTRA;",
    )
    .map_err(|_| storage())?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| storage())?;
    let empty = validate(&tx)?; // Another opener may have migrated while we waited.
    if empty {
        tx.execute_batch(V0).map_err(|_| storage())?;
    }
    let previous = identity(&tx)?.0;
    if previous < VERSION {
        let check: String = tx
            .query_row("PRAGMA quick_check", [], |r| r.get(0))
            .map_err(|_| storage())?;
        let foreign_errors: i64 = tx
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
                r.get(0)
            })
            .map_err(|_| storage())?;
        if check != "ok" || foreign_errors != 0 {
            return Err(unsupported());
        }
        if previous == 0 {
            tx.execute_batch(V1).map_err(|_| storage())?;
        }
        if previous < 2 {
            tx.execute_batch(V2).map_err(|_| storage())?;
        }
        if previous < 3 {
            tx.execute_batch(V3).map_err(|_| storage())?;
        }
        if previous < 4 {
            tx.execute_batch(V4).map_err(|_| storage())?;
        }
        if previous < 5 {
            tx.execute_batch(V5).map_err(|_| storage())?;
        }
        tx.execute_batch(V6).map_err(|_| storage())?;
        tx.pragma_update(None, "application_id", APPLICATION_ID)
            .map_err(|_| storage())?;
        tx.pragma_update(None, "user_version", VERSION)
            .map_err(|_| storage())?;
    }
    tx.commit().map_err(|_| storage())
}

/// Exact current identity/layout without migrations or persistent PRAGMA writes.
pub(super) fn validate_current(conn: &Connection) -> Result<()> {
    if identity(conn)? != (VERSION, APPLICATION_ID) || validate(conn)? {
        return Err(unsupported());
    }
    Ok(())
}
