//! Append-only human/provider observations. These are assertions, not transport
//! receipts, and cannot modify an attempt or grant permission to resend.
use crate::{
    Result, err, files::read_bounded, hash, integrity, invalid, now, smtp::package_lock, storage,
    store::Store,
};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Conclusion {
    Accepted,
    NotAccepted,
    Inconclusive,
    Received,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Source {
    ProviderLog,
    ProviderSupport,
    RecipientConfirmation,
    OwnerReview,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Evidence {
    schema_version: u32,
    package_id: String,
    attempt_id: String,
    message_id: String,
    manifest_sha256: String,
    observed_at: String,
    checked_by: String,
    source: Source,
    conclusion: Conclusion,
    evidence_ref: String,
    summary: String,
}
fn parse(raw: &[u8]) -> Result<Evidence> {
    let e: Evidence = serde_json::from_slice(raw).map_err(|_| invalid())?;
    if (matches!(e.conclusion, Conclusion::Received)
        && !matches!(e.source, Source::RecipientConfirmation))
        || e.schema_version != 1
        || chrono::DateTime::parse_from_rfc3339(&e.observed_at).is_err()
        || [&e.checked_by, &e.evidence_ref, &e.summary]
            .iter()
            .any(|s| s.trim().is_empty() || s.contains('\0') || s.len() > 8192)
    {
        return Err(invalid());
    }
    Ok(e)
}
fn check_binding(store: &Store, e: &Evidence) -> Result<()> {
    let p = store.load(&e.package_id)?;
    let found: Option<(String, String)> = store
        .conn
        .query_row(
            "SELECT message_id,outcome FROM attempts WHERE id=?1 AND package_id=?2",
            params![e.attempt_id, e.package_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|_| storage())?;
    let valid_attempt = found.is_some_and(|(message_id, outcome)| {
        message_id == e.message_id
            && (outcome == "unknown"
                || (outcome == "accepted" && matches!(e.conclusion, Conclusion::Received)))
    });
    if !valid_attempt
        || p.manifest.message_id != e.message_id
        || p.manifest_sha256 != e.manifest_sha256
    {
        return Err(err(
            "reconciliation_binding",
            "Evidence must match an unknown attempt, or a recipient report must match an accepted attempt",
            5,
        ));
    }
    Ok(())
}
struct Record {
    id: String,
    package_id: String,
    attempt_id: String,
    recorded_at: String,
    raw: Vec<u8>,
    digest: String,
    bytes: i64,
}
impl Record {
    fn from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get(0)?,
            package_id: r.get(1)?,
            attempt_id: r.get(2)?,
            recorded_at: r.get(3)?,
            raw: r.get(4)?,
            digest: r.get(5)?,
            bytes: r.get(6)?,
        })
    }
    fn verify(&self, store: &Store) -> Result<Evidence> {
        if self.raw.len() as i64 != self.bytes || hash(&self.raw) != self.digest {
            return Err(integrity());
        }
        let e = parse(&self.raw).map_err(|_| integrity())?;
        if e.package_id != self.package_id || e.attempt_id != self.attempt_id {
            return Err(integrity());
        }
        check_binding(store, &e).map_err(|_| integrity())?;
        Ok(e)
    }
    fn metadata(&self, e: &Evidence) -> Value {
        json!({"record_id":self.id,"package_id":self.package_id,"attempt_id":self.attempt_id,"recorded_at":self.recorded_at,"observed_at":e.observed_at,"source":e.source,"asserted_conclusion":e.conclusion,"evidence_sha256":self.digest,"evidence_bytes":self.bytes,"independently_verified":false,"retry_authorized":false})
    }
}
pub(super) fn record(store: &Store, id: &str, path: &Path) -> Result<Value> {
    record_kind(store, id, path, false)
}
pub(super) fn record_receipt(store: &Store, id: &str, path: &Path) -> Result<Value> {
    record_kind(store, id, path, true)
}
fn record_kind(store: &Store, id: &str, path: &Path, receipt: bool) -> Result<Value> {
    store.load(id)?;
    let _lock = package_lock(store, id)?;
    let raw = read_bounded(path, 65536)?;
    let e = parse(&raw)?;
    if matches!(e.conclusion, Conclusion::Received) != receipt {
        return Err(invalid());
    }
    if e.package_id != id {
        return Err(err(
            "reconciliation_binding",
            "Evidence package does not match selected package",
            5,
        ));
    }
    check_binding(store, &e)?;
    let state = store.status(id)?["state"].clone();
    let digest = hash(&raw);
    // Exact repeated evidence is idempotent, including concurrent CLI calls.
    let existing = store.conn.query_row("SELECT id,package_id,attempt_id,recorded_at,evidence,digest,bytes FROM reconciliations WHERE attempt_id=?1 AND digest=?2",params![e.attempt_id,digest],Record::from_row).optional().map_err(|_|storage())?;
    if let Some(row) = existing {
        let verified = row.verify(store)?;
        return Ok(
            json!({"record":row.metadata(&verified),"already_recorded":true,"state":state,"retry_authorized":false}),
        );
    }
    let row = Record {
        id: Uuid::new_v4().to_string(),
        package_id: id.into(),
        attempt_id: e.attempt_id.clone(),
        recorded_at: now(),
        bytes: raw.len() as i64,
        raw,
        digest,
    };
    store
        .conn
        .execute(
            "INSERT INTO reconciliations VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                row.id,
                row.package_id,
                row.attempt_id,
                row.recorded_at,
                row.raw,
                row.digest,
                row.bytes
            ],
        )
        .map_err(|_| storage())?;
    Ok(
        json!({"record":row.metadata(&e),"already_recorded":false,"state":state,"retry_authorized":false}),
    )
}
pub(super) fn list(store: &Store, id: &str) -> Result<Vec<Value>> {
    let mut stmt=store.conn.prepare("SELECT id,package_id,attempt_id,recorded_at,evidence,digest,bytes FROM reconciliations WHERE package_id=?1 ORDER BY rowid").map_err(|_|storage())?;
    let rows = stmt
        .query_map([id], Record::from_row)
        .map_err(|_| storage())?;
    rows.map(|row| {
        let r = row.map_err(|_| storage())?;
        let e = r.verify(store)?;
        Ok(r.metadata(&e))
    })
    .collect()
}
pub(super) fn inspect(store: &Store, id: &str) -> Result<Value> {
    let r=store.conn.query_row("SELECT id,package_id,attempt_id,recorded_at,evidence,digest,bytes FROM reconciliations WHERE id=?1",[id],Record::from_row).optional().map_err(|_|storage())?.ok_or_else(||err("not_found","Reconciliation record not found",3))?;
    let e = r.verify(store)?;
    Ok(json!({"record":r.metadata(&e),"evidence":e,"integrity":"verified"}))
}
