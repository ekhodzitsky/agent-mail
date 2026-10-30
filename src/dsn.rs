//! Immutable DSN policy, evidence persistence and status interpretation.
mod report;
#[cfg(test)]
mod tests;
mod transport;
use crate::{
    Result, err,
    files::{MAX_TEXT, read_bounded},
    hash, integrity, now,
    package::Package,
    smtp::package_lock,
    storage,
    store::Store,
};
use clap::ValueEnum;
use report::{parse_report, parse_report_mode};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;
pub(super) use transport::submit;
#[cfg(test)]
pub(super) use transport::submit_with_tls;
use uuid::Uuid;

#[derive(Clone, Copy, Serialize, Deserialize, ValueEnum, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum DsnMode {
    Off,
    Optional,
    Required,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DsnPolicy {
    pub mode: DsnMode,
    notify: String,
    ret: String,
    envid_prefix: String,
}
impl DsnPolicy {
    pub fn new(mode: DsnMode, id: &str) -> Self {
        Self {
            mode,
            notify: "SUCCESS,FAILURE,DELAY".into(),
            ret: "HDRS".into(),
            envid_prefix: id.into(),
        }
    }
    pub fn valid(&self, id: &str) -> bool {
        self.mode != DsnMode::Off
            && self.notify == "SUCCESS,FAILURE,DELAY"
            && self.ret == "HDRS"
            && self.envid_prefix == id
            && Uuid::parse_str(id).is_ok()
    }
}
fn envid(package: &str, attempt: &str) -> String {
    format!("{package}.{attempt}")
}
fn event(store: &Store, attempt: &str, kind: &str, payload: Value) -> Result<()> {
    let raw = serde_json::to_vec(&payload).map_err(|_| storage())?;
    store
        .conn
        .execute(
            "INSERT INTO dsn_events VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                Uuid::new_v4().to_string(),
                attempt,
                kind,
                now(),
                raw,
                hash(&raw)
            ],
        )
        .map_err(|_| storage())?;
    Ok(())
}
fn bind_mode(store: &Store, id: &str, attempt: &str, by_headers: bool) -> Result<Package> {
    let p = store.load(id)?;
    if !by_headers && p.manifest.dsn.is_none() {
        return Err(err("dsn_binding", "Package has no frozen DSN policy", 5));
    }
    let outcome: Option<String> = store
        .conn
        .query_row(
            "SELECT outcome FROM attempts WHERE id=?1 AND package_id=?2",
            params![attempt, id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| storage())?;
    if !outcome.is_some_and(|s| s == "accepted" || s == "unknown") {
        return Err(err(
            "dsn_binding",
            "Report must bind to an accepted or unresolved attempt",
            5,
        ));
    }
    if by_headers {
        let (count, matching): (i64, i64) = store
            .conn
            .query_row(
                "SELECT count(*),coalesce(sum(message_id=?2),0) FROM attempts WHERE package_id=?1",
                params![id, p.manifest.message_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|_| storage())?;
        if count != 1 || matching != 1 {
            return Err(err(
                "dsn_binding",
                "Header correlation requires one unambiguous original attempt",
                5,
            ));
        }
        return Ok(p);
    }
    let planned: Option<(Vec<u8>,String)> = store.conn.query_row("SELECT payload,digest FROM dsn_events WHERE attempt_id=?1 AND kind='parameters_about_to_send'",[attempt],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|_|storage())?;
    let (raw, digest) = planned.ok_or_else(|| {
        err(
            "dsn_binding",
            "No DSN parameter use recorded for this attempt",
            5,
        )
    })?;
    if hash(&raw) != digest {
        return Err(integrity());
    }
    let value: Value = serde_json::from_slice(&raw).map_err(|_| integrity())?;
    if value != json!({"dsn_parameters":true}) {
        return Err(err(
            "dsn_binding",
            "DSN parameters were not used for this attempt",
            5,
        ));
    }
    Ok(p)
}
#[cfg(test)]
pub(super) fn import(store: &Store, id: &str, attempt: &str, path: &Path) -> Result<Value> {
    import_mode(store, id, attempt, path, false)
}
pub(super) fn import_mode(
    store: &Store,
    id: &str,
    attempt: &str,
    path: &Path,
    by_headers: bool,
) -> Result<Value> {
    store.load(id)?;
    let _lock = package_lock(store, id)?;
    status(store, id)?; // Verify existing events/reports before appending.
    let p = bind_mode(store, id, attempt, by_headers)?;
    let raw = read_bounded(path, MAX_TEXT)?;
    let report = if by_headers {
        parse_report_mode(&raw, id, attempt, &p.manifest.recipient, Some(&p.manifest))?
    } else {
        parse_report(&raw, id, attempt, &p.manifest.recipient)?
    };
    let table = if by_headers {
        "dsn_header_reports"
    } else {
        "dsn_reports"
    };
    let digest = hash(&raw);
    let existing: Option<String> = store
        .conn
        .query_row(
            &format!("SELECT id FROM {table} WHERE attempt_id=?1 AND digest=?2"),
            params![attempt, digest],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| storage())?;
    let duplicate = existing.is_some();
    let report_id = existing.unwrap_or_else(|| Uuid::new_v4().to_string());
    if !duplicate {
        store
            .conn
            .execute(
                &format!("INSERT INTO {table} VALUES(?1,?2,?3,?4,?5,?6,?7)"),
                params![report_id, id, attempt, now(), raw, digest, raw.len() as i64],
            )
            .map_err(|_| storage())?;
    }
    Ok(
        json!({"report_id":report_id,"report":report,"already_recorded":duplicate,"correlation":if by_headers {"original_headers"} else {"envid"},"network_used":false}),
    )
}
fn verified_report(
    store: &Store,
    identity: (&str, &str, &str),
    data: (&[u8], &str, i64, &str),
    by_headers: bool,
) -> Result<Value> {
    let (id, package, attempt) = identity;
    let (raw, digest, bytes, recorded) = data;
    if raw.len() as i64 != bytes || hash(raw) != digest {
        return Err(integrity());
    }
    let p = bind_mode(store, package, attempt, by_headers).map_err(|_| integrity())?;
    let report = parse_report_mode(
        raw,
        package,
        attempt,
        &p.manifest.recipient,
        by_headers.then_some(&p.manifest),
    )
    .map_err(|_| integrity())?;
    Ok(
        json!({"report_id":id,"package_id":package,"attempt_id":attempt,"recorded_at":recorded,"sha256":digest,"bytes":bytes,"report":report,"correlation":if by_headers {"original_headers"} else {"envid"}}),
    )
}
pub(super) fn status(store: &Store, id: &str) -> Result<Value> {
    let mut stmt=store.conn.prepare("SELECT e.attempt_id,e.kind,e.recorded_at,e.payload,e.digest FROM dsn_events e JOIN attempts a ON a.id=e.attempt_id WHERE a.package_id=?1 ORDER BY e.rowid").map_err(|_|storage())?;
    let rows = stmt
        .query_map([id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Vec<u8>>(3)?,
                r.get::<_, String>(4)?,
            ))
        })
        .map_err(|_| storage())?;
    let mut events = Vec::new();
    for row in rows {
        let (a, k, t, raw, digest) = row.map_err(|_| storage())?;
        if hash(&raw) != digest {
            return Err(integrity());
        }
        let value: Value = serde_json::from_slice(&raw).map_err(|_| integrity())?;
        events.push(json!({"attempt_id":a,"kind":k,"recorded_at":t,"details":value}));
    }
    let mut reports = Vec::new();
    for (table, by_headers) in [("dsn_reports", false), ("dsn_header_reports", true)] {
        let mut stmt=store.conn.prepare(&format!("SELECT id,attempt_id,recorded_at,raw,digest,bytes FROM {table} WHERE package_id=?1 ORDER BY rowid")).map_err(|_|storage())?;
        let rows = stmt
            .query_map([id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Vec<u8>>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, i64>(5)?,
                ))
            })
            .map_err(|_| storage())?;
        for row in rows {
            let (r, a, t, raw, digest, bytes) = row.map_err(|_| storage())?;
            reports.push(verified_report(
                store,
                (&r, id, &a),
                (&raw, &digest, bytes, &t),
                by_headers,
            )?);
        }
    }
    Ok(json!({"events":events,"reports":reports}))
}
pub(super) fn delivery_observation(dsn: &Value) -> Result<Value> {
    let reports = dsn["reports"].as_array().ok_or_else(integrity)?;
    let has = |action| reports.iter().any(|r| r["report"]["action"] == action);
    let outcome = if has("failed") && has("delivered") {
        "conflicting_reports"
    } else if has("failed") {
        "failed_reported"
    } else if has("delivered") {
        "delivered_reported"
    } else if has("delayed") {
        "delayed_reported"
    } else if has("relayed") {
        "relayed_reported"
    } else if has("expanded") {
        "expanded_reported"
    } else {
        "not_reported"
    };
    let ids: Vec<_> = reports.iter().map(|r| r["report_id"].clone()).collect();
    Ok(
        json!({"outcome":outcome,"report_ids":ids,"independently_verified":false,"retry_authorized":false}),
    )
}
pub(super) fn inspect(store: &Store, id: &str) -> Result<Value> {
    for (table, by_headers) in [("dsn_reports", false), ("dsn_header_reports", true)] {
        let row=store.conn.query_row(&format!("SELECT package_id,attempt_id,recorded_at,raw,digest,bytes FROM {table} WHERE id=?1"),[id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,Vec<u8>>(3)?,r.get::<_,String>(4)?,r.get::<_,i64>(5)?))).optional().map_err(|_|storage())?;
        let Some((p, a, t, raw, digest, bytes)) = row else {
            continue;
        };
        return Ok(
            json!({"record":verified_report(store,(id,&p,&a),(&raw,&digest,bytes,&t),by_headers)?,"integrity":"verified"}),
        );
    }
    Err(err("not_found", "DSN report not found", 3))
}
