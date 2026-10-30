//! Opt-in keyset pages; legacy listing retains full verification and shape.
use crate::{Result, err, storage, store::Store};
use rusqlite::OptionalExtension;
use serde_json::{Value, json};

pub(super) fn list(
    store: &Store,
    limit: Option<u16>,
    after: Option<&str>,
    summary: bool,
) -> Result<Value> {
    // Keep selection and status reads in one SQLite read snapshot per invocation.
    let tx = store.conn.unchecked_transaction().map_err(|_| storage())?;
    let start: i64 = if let Some(id) = after {
        tx.query_row("SELECT rowid FROM packages WHERE id=?1", [id], |r| r.get(0))
            .optional()
            .map_err(|_| storage())?
            .ok_or_else(|| err("not_found", "Continuation package not found", 3))?
    } else {
        0
    };
    let count = limit.map(|n| i64::from(n) + 1).unwrap_or(-1);
    let mut statement = tx
        .prepare("SELECT id FROM packages WHERE rowid>?1 ORDER BY rowid LIMIT ?2")
        .map_err(|_| storage())?;
    let mut ids = statement
        .query_map(rusqlite::params![start, count], |r| r.get::<_, String>(0))
        .map_err(|_| storage())?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| storage())?;
    let more = limit.is_some_and(|n| ids.len() > usize::from(n));
    if more {
        ids.pop();
    }
    let values = ids
        .iter()
        .map(|id| {
            if summary {
                Ok(json!({"package_id": id, "integrity":"not_checked"}))
            } else {
                store.status(id)
            }
        })
        .collect::<Result<Vec<_>>>()?;
    let mut result = json!({"packages": values});
    if let Some(limit) = limit {
        result["pagination"] = json!({"limit":limit, "has_more":more,
            "next_after":if more { ids.last() } else { None },
            "mode":if summary { "ids_only" } else { "verified_status" }});
    }
    drop(statement);
    tx.commit().map_err(|_| storage())?;
    Ok(result)
}
