#![allow(clippy::unwrap_used, clippy::expect_used)] // Synthetic test fixtures only.
use super::*;
use crate::{config::Config, package::prepare, smtp::send_using};
use lettre::transport::smtp::{Error as SmtpError, response::Response};
use lettre::{Transport, address::Envelope};
use rusqlite::{Connection, params};
use std::fs;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

pub(super) fn fixture() -> (tempfile::TempDir, Store, Config, String) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(dir.path().to_path_buf()).unwrap();
    let cfg = Config {
        sender: "sender@example.org".into(),
        smtp: None,
    };
    let id = prepare(
        &mut store,
        &cfg,
        "recipient@example.org".into(),
        "Synthetic".into(),
        "Final letter\nSignature".into(),
        vec![("resume.pdf".into(), b"%PDF-1.4\nsynthetic".to_vec())],
        json!({}),
    )
    .unwrap()["package_id"]
        .as_str()
        .unwrap()
        .to_string();
    (dir, store, cfg, id)
}
struct Fake(Arc<AtomicUsize>);
impl Transport for Fake {
    type Ok = Response;
    type Error = SmtpError;
    fn send_raw(&self, _: &Envelope, _: &[u8]) -> std::result::Result<Response, SmtpError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok("250 accepted\r\n".parse().unwrap())
    }
}
fn attempt_count(conn: &Connection) -> i64 {
    conn.query_row("SELECT count(*) FROM attempts", [], |r| r.get(0))
        .unwrap()
}
fn snapshot(conn: &Connection) -> Vec<Vec<rusqlite::types::Value>> {
    let mut result = Vec::new();
    for table in ["packages", "attachments", "attempts"] {
        let mut stmt = conn
            .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
            .unwrap();
        let cols = stmt.column_count();
        let rows = stmt
            .query_map([], |r| (0..cols).map(|i| r.get(i)).collect())
            .unwrap();
        result.extend(rows.map(|r| r.unwrap()));
    }
    result
}
#[test]
fn migration_preserves_bytes_attempts_and_resend_blocks() {
    for outcome in ["unknown", "accepted", "rejected"] {
        let (dir, store, cfg, id) = fixture();
        let finished = if outcome == "unknown" {
            None
        } else {
            Some("2026-01-01T00:01:00+00:00")
        };
        store.conn.execute("INSERT INTO attempts VALUES('a',?1,'2026-01-01T00:00:00+00:00',?2,?3,'<synthetic@example.org>',NULL)",params![id,finished,outcome]).unwrap();
        let before = snapshot(&store.conn);
        store.conn.execute_batch("DROP TABLE dsn_header_reports; DROP TABLE prepare_operations; DROP TABLE dsn_reports; DROP TABLE dsn_events; DROP TABLE reconciliations; DROP TRIGGER attempts_no_delete; DROP TRIGGER attempts_final_once; PRAGMA user_version=0; PRAGMA application_id=0;").unwrap();
        drop(store);
        let migrated = Store::open(dir.path().to_path_buf()).unwrap();
        assert_eq!(snapshot(&migrated.conn), before);
        assert_eq!(
            migrated
                .conn
                .pragma_query_value(None, "user_version", |r| r.get::<_, i32>(0))
                .unwrap(),
            schema::VERSION
        );
        assert_eq!(migrated.status(&id).unwrap()["state"], outcome);
        if outcome != "rejected" {
            assert_eq!(
                send(&migrated, &cfg, &id, false).unwrap_err().code,
                "send_blocked"
            );
        }
        drop(migrated);
        let reopened = Store::open(dir.path().to_path_buf()).unwrap();
        assert_eq!(snapshot(&reopened.conn), before);
    }
}
#[test]
fn schema_rejection_is_fail_closed_and_does_not_repair_data() {
    for edit in [
        "PRAGMA user_version=999",
        "PRAGMA application_id=123",
        "DROP INDEX one_unresolved_submission",
        "ALTER TABLE attempts ADD COLUMN unexpected TEXT",
    ] {
        let (dir, store, _, _) = fixture();
        store.conn.execute_batch(edit).unwrap();
        drop(store);
        let path = dir.path().join("mail.sqlite3");
        let before = fs::read(&path).unwrap();
        assert_eq!(
            Store::open(dir.path().to_path_buf()).err().unwrap().code,
            "storage_schema"
        );
        assert_eq!(fs::read(path).unwrap(), before);
    }
    let dir = tempfile::tempdir().unwrap();
    let conn = Connection::open(dir.path().join("mail.sqlite3")).unwrap();
    conn.execute_batch("CREATE TABLE unrelated(data BLOB)")
        .unwrap();
    drop(conn);
    assert_eq!(
        Store::open(dir.path().to_path_buf()).err().unwrap().code,
        "storage_schema"
    );
}
#[test]
fn migration_commit_failure_rolls_back_schema_and_version() {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(include_str!("schema-v0.sql")).unwrap();
    conn.commit_hook(Some(|| true)).unwrap();
    assert_eq!(schema::initialize(&mut conn).unwrap_err().code, "storage");
    assert_eq!(
        conn.pragma_query_value(None, "user_version", |r| r.get::<_, i32>(0))
            .unwrap(),
        0
    );
    let count: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_schema WHERE name='attempts_final_once'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
    conn.commit_hook(None::<fn() -> bool>).unwrap();
    schema::initialize(&mut conn).unwrap();
}
#[test]
fn failed_attempt_commit_never_invokes_transport() {
    let (dir, store, cfg, id) = fixture();
    let calls = Arc::new(AtomicUsize::new(0));
    store.conn.commit_hook(Some(|| true)).unwrap();
    let error = send_using(&store, &cfg, &id, false, |_| Ok(Fake(calls.clone()))).unwrap_err();
    assert_eq!(error.code, "storage");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(attempt_count(&store.conn), 0);
    drop(store);
    assert_eq!(
        attempt_count(&Store::open(dir.path().to_path_buf()).unwrap().conn),
        0
    );
}
#[test]
fn failed_final_commit_keeps_unknown_and_blocks_retry_after_reopen() {
    let (dir, store, cfg, id) = fixture();
    let calls = Arc::new(AtomicUsize::new(0));
    let commits = Arc::new(AtomicUsize::new(0));
    let c = commits.clone();
    store
        .conn
        .commit_hook(Some(move || c.fetch_add(1, Ordering::SeqCst) == 1))
        .unwrap();
    assert_eq!(
        send_using(&store, &cfg, &id, false, |_| Ok(Fake(calls.clone())))
            .unwrap_err()
            .code,
        "storage"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(commits.load(Ordering::SeqCst), 2);
    drop(store);
    let reopened = Store::open(dir.path().to_path_buf()).unwrap();
    let status = reopened.status(&id).unwrap();
    assert_eq!(status["state"], "unknown");
    assert_eq!(status["attempts"][0]["finished_at"], Value::Null);
    assert_eq!(
        send_using(&reopened, &cfg, &id, false, |_| Ok(Fake(calls.clone())))
            .unwrap_err()
            .code,
        "send_blocked"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
#[test]
fn sqlite_full_during_attempt_record_prevents_network() {
    let (_dir, store, cfg, id) = fixture();
    let calls = Arc::new(AtomicUsize::new(0));
    // SQLite's real page-allocation failure, confined to this temporary database.
    store.conn.execute_batch("CREATE TABLE fault_payload(payload BLOB); CREATE TRIGGER fault_full BEFORE INSERT ON attempts BEGIN INSERT INTO fault_payload VALUES(zeroblob(1048576)); END;").unwrap();
    let pages: i64 = store
        .conn
        .pragma_query_value(None, "page_count", |r| r.get(0))
        .unwrap();
    store
        .conn
        .pragma_update(None, "max_page_count", pages)
        .unwrap();
    let failure = store
        .conn
        .execute("INSERT INTO fault_payload VALUES(zeroblob(1048576))", [])
        .unwrap_err();
    assert_eq!(
        failure.sqlite_error_code(),
        Some(rusqlite::ErrorCode::DiskFull)
    );
    assert_eq!(
        send_using(&store, &cfg, &id, false, |_| Ok(Fake(calls.clone())))
            .unwrap_err()
            .code,
        "storage"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(attempt_count(&store.conn), 0);
}
#[test]
fn sqlite_full_during_preparation_leaves_no_partial_package() {
    let (_dir, mut store, cfg, _) = fixture();
    let before = snapshot(&store.conn);
    let pages: i64 = store
        .conn
        .pragma_query_value(None, "page_count", |r| r.get(0))
        .unwrap();
    store
        .conn
        .pragma_update(None, "max_page_count", pages)
        .unwrap();
    let mut pdf = b"%PDF-1.4\n".to_vec();
    pdf.resize(1024 * 1024, b'x');
    assert_eq!(
        prepare(
            &mut store,
            &cfg,
            "recipient@example.org".into(),
            "Synthetic".into(),
            "Body".into(),
            vec![("resume.pdf".into(), pdf)],
            json!({})
        )
        .unwrap_err()
        .code,
        "storage"
    );
    assert_eq!(snapshot(&store.conn), before);
}
#[test]
fn completed_attempt_history_cannot_be_rewritten_or_deleted() {
    let (_dir, store, cfg, id) = fixture();
    let calls = Arc::new(AtomicUsize::new(0));
    send_using(&store, &cfg, &id, false, |_| Ok(Fake(calls))).unwrap();
    assert!(
        store
            .conn
            .execute("UPDATE attempts SET outcome='rejected'", [])
            .is_err()
    );
    assert!(store.conn.execute("DELETE FROM attempts", []).is_err());
    assert_eq!(store.status(&id).unwrap()["state"], "accepted");
}

#[test]
fn sqlite_full_during_final_result_keeps_durable_unknown() {
    let (dir, store, cfg, id) = fixture();
    store
        .conn
        .execute_batch("CREATE TABLE fault_payload(payload BLOB);")
        .unwrap();
    struct AcceptedThenFull<'a>(&'a Connection);
    impl Transport for AcceptedThenFull<'_> {
        type Ok = Response;
        type Error = SmtpError;
        fn send_raw(&self, _: &Envelope, _: &[u8]) -> std::result::Result<Response, SmtpError> {
            // The attempt has committed before the simulated server acceptance.
            assert_eq!(attempt_count(self.0), 1);
            self.0.execute_batch("CREATE TRIGGER fault_full BEFORE UPDATE ON attempts BEGIN INSERT INTO fault_payload VALUES(zeroblob(1048576)); END;").unwrap();
            let pages: i64 = self
                .0
                .pragma_query_value(None, "page_count", |r| r.get(0))
                .unwrap();
            self.0.pragma_update(None, "max_page_count", pages).unwrap();
            let probe = self
                .0
                .execute("INSERT INTO fault_payload VALUES(zeroblob(1048576))", [])
                .unwrap_err();
            assert_eq!(
                probe.sqlite_error_code(),
                Some(rusqlite::ErrorCode::DiskFull)
            );
            Ok("250 accepted\r\n".parse().unwrap())
        }
    }
    assert_eq!(
        send_using(&store, &cfg, &id, false, |_| Ok(AcceptedThenFull(
            &store.conn
        )))
        .unwrap_err()
        .code,
        "storage"
    );
    assert_eq!(store.status(&id).unwrap()["state"], "unknown");
    // Remove only the test apparatus, never the package or attempt evidence.
    store
        .conn
        .execute_batch("DROP TRIGGER fault_full; DROP TABLE fault_payload;")
        .unwrap();
    drop(store);
    let reopened = Store::open(dir.path().to_path_buf()).unwrap();
    assert_eq!(
        reopened.status(&id).unwrap()["attempts"][0]["finished_at"],
        Value::Null
    );
    assert_eq!(
        send(&reopened, &cfg, &id, false).unwrap_err().code,
        "send_blocked"
    );
}

#[test]
fn concurrent_legacy_openers_complete_one_consistent_migration() {
    let (dir, store, _, _) = fixture();
    let before = snapshot(&store.conn);
    store.conn.execute_batch("DROP TABLE dsn_header_reports; DROP TABLE prepare_operations; DROP TABLE dsn_reports; DROP TABLE dsn_events; DROP TABLE reconciliations; DROP TRIGGER attempts_no_delete; DROP TRIGGER attempts_final_once; PRAGMA user_version=0; PRAGMA application_id=0;").unwrap();
    drop(store);
    let barrier = Arc::new(std::sync::Barrier::new(4));
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let barrier = barrier.clone();
            let home = dir.path().to_path_buf();
            std::thread::spawn(move || {
                barrier.wait();
                let store = Store::open(home).unwrap();
                snapshot(&store.conn)
            })
        })
        .collect();
    for h in handles {
        assert_eq!(h.join().unwrap(), before);
    }
}

#[test]
fn schema_one_upgrades_without_rewriting_package_or_attempt_rows() {
    let (dir, store, _, _) = fixture();
    let before = snapshot(&store.conn);
    store
        .conn
        .execute_batch("DROP TABLE dsn_header_reports; DROP TABLE prepare_operations; DROP TABLE dsn_reports; DROP TABLE dsn_events; DROP TABLE reconciliations; PRAGMA user_version=1;")
        .unwrap();
    drop(store);
    let migrated = Store::open(dir.path().to_path_buf()).unwrap();
    assert_eq!(snapshot(&migrated.conn), before);
    assert_eq!(
        migrated
            .conn
            .pragma_query_value(None, "user_version", |r| r.get::<_, i32>(0))
            .unwrap(),
        schema::VERSION
    );
}
#[test]
fn failed_reconciliation_commit_leaves_unknown_without_partial_record() {
    let (dir, store, cfg, id) = fixture();
    let p = store.load(&id).unwrap();
    store.conn.execute("INSERT INTO attempts VALUES('synthetic-attempt',?1,'2026-01-01T00:00:00+00:00',NULL,'unknown',?2,NULL)",params![id,p.manifest.message_id]).unwrap();
    let evidence = json!({"schema_version":1,"package_id":id,"attempt_id":"synthetic-attempt","message_id":p.manifest.message_id,"manifest_sha256":p.manifest_sha256,"observed_at":"2026-01-02T00:00:00+00:00","checked_by":"synthetic","source":"owner_review","conclusion":"inconclusive","evidence_ref":"synthetic","summary":"No definitive evidence"});
    let path = dir.path().join("evidence.json");
    fs::write(&path, serde_json::to_vec(&evidence).unwrap()).unwrap();
    store.conn.commit_hook(Some(|| true)).unwrap();
    assert_eq!(
        reconciliation::record(&store, &id, &path).unwrap_err().code,
        "storage"
    );
    drop(store);
    let reopened = Store::open(dir.path().to_path_buf()).unwrap();
    assert_eq!(reopened.status(&id).unwrap()["reconciliations"], json!([]));
    assert_eq!(
        send(&reopened, &cfg, &id, false).unwrap_err().code,
        "send_blocked"
    );
}

#[test]
fn schema_two_upgrade_preserves_existing_evidence_and_attempts() {
    let (dir, store, _, id) = fixture();
    let p = store.load(&id).unwrap();
    store.conn.execute("INSERT INTO attempts VALUES('synthetic-attempt',?1,'2026-01-01T00:00:00+00:00',NULL,'unknown',?2,NULL)",params![id,p.manifest.message_id]).unwrap();
    let evidence = json!({"schema_version":1,"package_id":id,"attempt_id":"synthetic-attempt","message_id":p.manifest.message_id,"manifest_sha256":p.manifest_sha256,"observed_at":"2026-01-02T00:00:00+00:00","checked_by":"synthetic","source":"owner_review","conclusion":"inconclusive","evidence_ref":"synthetic","summary":"No definitive evidence"});
    let path = dir.path().join("evidence.json");
    fs::write(&path, serde_json::to_vec(&evidence).unwrap()).unwrap();
    reconciliation::record(&store, &id, &path).unwrap();
    let before = store.status(&id).unwrap();
    let package_before = snapshot(&store.conn);
    store
        .conn
        .execute_batch(
            "DROP TABLE dsn_header_reports; DROP TABLE prepare_operations; DROP TABLE dsn_reports; DROP TABLE dsn_events;",
        )
        .unwrap();
    let old = Connection::open_in_memory().unwrap();
    old.execute_batch(include_str!("schema-v0.sql")).unwrap();
    old.execute_batch(include_str!("schema-v2.sql")).unwrap();
    let trigger: String = old
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE name='reconciliations_unknown_only'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    store
        .conn
        .execute_batch("DROP TRIGGER reconciliations_attempt_only;")
        .unwrap();
    store.conn.execute_batch(&trigger).unwrap();
    store.conn.pragma_update(None, "user_version", 2).unwrap();
    drop(store);
    let mut veto = Connection::open(dir.path().join("mail.sqlite3")).unwrap();
    veto.commit_hook(Some(|| true)).unwrap();
    assert!(schema::initialize(&mut veto).is_err());
    assert_eq!(
        veto.pragma_query_value(None, "user_version", |r| r.get::<_, i32>(0))
            .unwrap(),
        2
    );
    drop(veto);
    let migrated = Store::open(dir.path().to_path_buf()).unwrap();
    assert_eq!(migrated.status(&id).unwrap(), before);
    assert_eq!(snapshot(&migrated.conn), package_before);
    assert_eq!(
        migrated
            .conn
            .pragma_query_value(None, "user_version", |r| r.get::<_, i32>(0))
            .unwrap(),
        schema::VERSION
    );
}

#[test]
fn failed_network_worker_launch_creates_no_attempt() {
    let (dir, store, mut cfg, id) = fixture();
    cfg.smtp = Some(crate::config::Smtp {
        host: "smtp.example.invalid".into(),
        port: 465,
        security: "tls".into(),
        username: None,
        password_env: None,
        timeout_seconds: 1,
        total_timeout_seconds: 1,
    });
    let result = crate::smtp::supervised_send_with_executable(
        &store,
        &cfg,
        &dir.path().join("config.toml"),
        &id,
        &dir.path().join("missing-binary"),
    );
    assert_eq!(result.unwrap_err().code, "storage");
    assert_eq!(store.status(&id).unwrap()["attempts"], json!([]));
}

fn dsn_fixture() -> (tempfile::TempDir, Store, Config, String, String) {
    let (dir, mut store, cfg, _) = fixture();
    let package = crate::package::prepare_content(
        &mut store,
        &cfg,
        "recipient@example.org".into(),
        "Synthetic".into(),
        crate::package::Content {
            body: "Synthetic body".into(),
            html: None,
            dsn: Some(crate::dsn::DsnMode::Required),
            operation_key: None,
        },
        vec![("test.pdf".into(), b"%PDF-1.4 synthetic".to_vec())],
        json!({}),
    )
    .unwrap();
    let id = package["package_id"].as_str().unwrap().to_string();
    let attempt = "synthetic-dsn-attempt".to_string();
    store
        .conn
        .execute(
            "INSERT INTO attempts VALUES(?1,?2,'2026-01-01T00:00:00+00:00',NULL,'unknown',?3,NULL)",
            params![
                attempt,
                id,
                package["manifest"]["message_id"].as_str().unwrap()
            ],
        )
        .unwrap();
    (dir, store, cfg, id, attempt)
}
#[test]
fn dsn_event_commit_failure_prevents_connection_setup() {
    let (_dir, store, mut cfg, id, attempt) = dsn_fixture();
    cfg.smtp = Some(crate::config::Smtp {
        host: "smtp.example.invalid".into(),
        port: 465,
        security: "tls".into(),
        username: None,
        password_env: None,
        timeout_seconds: 1,
        total_timeout_seconds: 1,
    });
    let p = store.load(&id).unwrap();
    store.conn.commit_hook(Some(|| true)).unwrap();
    let tls_called = std::cell::Cell::new(false);
    let result = crate::dsn::submit_with_tls(
        &store,
        &cfg,
        &p,
        &attempt,
        |host| {
            tls_called.set(true);
            lettre::transport::smtp::client::TlsParameters::new(host.into())
        },
        |_| Ok("synthetic".into()),
    );
    assert_eq!(result.err().unwrap().code, "storage");
    assert!(!tls_called.get());
    assert_eq!(store.status(&id).unwrap()["state"], "unknown");
    assert_eq!(store.status(&id).unwrap()["dsn"]["events"], json!([]));
}
#[test]
fn dsn_report_commit_failure_and_corruption_preserve_original_attempt() {
    let (dir, store, _, id, attempt) = dsn_fixture();
    let payload = br#"{"dsn_parameters":true}"#;
    store.conn.execute("INSERT INTO dsn_events VALUES('event',?1,'parameters_about_to_send','2026-01-01T00:00:00+00:00',?2,?3)",params![attempt,payload,hash(payload)]).unwrap();
    let report = format!(
        "Content-Type: multipart/report; report-type=delivery-status; boundary=x\r\n\r\n--x\r\nContent-Type: text/plain\r\n\r\nSynthetic report\r\n--x\r\nContent-Type: message/delivery-status\r\n\r\nReporting-MTA: dns; example.org\r\nOriginal-Envelope-Id: {id}.{attempt}\r\n\r\nFinal-Recipient: rfc822; recipient@example.org\r\nAction: delivered\r\nStatus: 2.0.0\r\n\r\n--x--\r\n"
    );
    let path = dir.path().join("report.eml");
    fs::write(&path, report).unwrap();
    let before = store.status(&id).unwrap()["attempts"].clone();
    store.conn.commit_hook(Some(|| true)).unwrap();
    assert_eq!(
        crate::dsn::import(&store, &id, &attempt, &path)
            .unwrap_err()
            .code,
        "storage"
    );
    store.conn.commit_hook(None::<fn() -> bool>).unwrap();
    assert_eq!(store.status(&id).unwrap()["dsn"]["reports"], json!([]));
    let result = crate::dsn::import(&store, &id, &attempt, &path).unwrap();
    let rid = result["report_id"].as_str().unwrap();
    assert_eq!(store.status(&id).unwrap()["attempts"], before);
    let trigger: String = store
        .conn
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE name='dsn_reports_no_update'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    store
        .conn
        .execute_batch("DROP TRIGGER dsn_reports_no_update; UPDATE dsn_reports SET raw=x'00';")
        .unwrap();
    store.conn.execute_batch(&trigger).unwrap();
    assert_eq!(
        crate::dsn::inspect(&store, rid).unwrap_err().code,
        "integrity"
    );
    assert_eq!(store.status(&id).unwrap_err().code, "integrity");
}
#[test]
fn schema_three_upgrade_is_atomic_and_preserves_evidence() {
    let (dir, store, _, _) = fixture();
    let before = snapshot(&store.conn);
    store
        .conn
        .execute_batch("DROP TABLE dsn_header_reports; DROP TABLE prepare_operations; DROP TABLE dsn_reports; DROP TABLE dsn_events; PRAGMA user_version=3;")
        .unwrap();
    drop(store);
    let mut veto = Connection::open(dir.path().join("mail.sqlite3")).unwrap();
    veto.commit_hook(Some(|| true)).unwrap();
    assert!(schema::initialize(&mut veto).is_err());
    assert_eq!(
        veto.pragma_query_value(None, "user_version", |r| r.get::<_, i32>(0))
            .unwrap(),
        3
    );
    drop(veto);
    let upgraded = Store::open(dir.path().to_path_buf()).unwrap();
    assert_eq!(snapshot(&upgraded.conn), before);
    assert_eq!(
        upgraded
            .conn
            .pragma_query_value(None, "user_version", |r| r.get::<_, i32>(0))
            .unwrap(),
        schema::VERSION
    );
}

#[test]
fn schema_four_upgrade_preserves_history_and_rolls_back_failed_migration() {
    let (dir, store, _, id) = fixture();
    let package = store.load(&id).unwrap();
    store.conn.execute("INSERT INTO attempts VALUES('synthetic',?1,'2026-01-01T00:00:00+00:00',NULL,'unknown',?2,NULL)",params![id,package.manifest.message_id]).unwrap();
    let before = snapshot(&store.conn);
    store
        .conn
        .execute_batch(
            "DROP TABLE dsn_header_reports; DROP TABLE prepare_operations; PRAGMA user_version=4;",
        )
        .unwrap();
    drop(store);
    let mut veto = Connection::open(dir.path().join("mail.sqlite3")).unwrap();
    veto.commit_hook(Some(|| true)).unwrap();
    assert!(schema::initialize(&mut veto).is_err());
    assert_eq!(
        veto.pragma_query_value(None, "user_version", |r| r.get::<_, i32>(0))
            .unwrap(),
        4
    );
    assert_eq!(snapshot(&veto), before);
    drop(veto);
    let migrated = Store::open(dir.path().to_path_buf()).unwrap();
    assert_eq!(snapshot(&migrated.conn), before);
    assert_eq!(migrated.status(&id).unwrap()["state"], "unknown");
    assert_eq!(
        send(
            &migrated,
            &Config {
                sender: "sender@example.org".into(),
                smtp: None
            },
            &id,
            false
        )
        .unwrap_err()
        .code,
        "send_blocked"
    );
}

#[test]
fn operation_and_package_commit_or_roll_back_together() {
    let (_dir, mut store, cfg, _) = fixture();
    let before = snapshot(&store.conn);
    store.conn.commit_hook(Some(|| true)).unwrap();
    let prepared = crate::package::prepare_content(
        &mut store,
        &cfg,
        "recipient@example.org".into(),
        "Synthetic".into(),
        crate::package::Content {
            body: "Synthetic body".into(),
            html: None,
            dsn: None,
            operation_key: Some("synthetic-operation".into()),
        },
        vec![("resume.pdf".into(), b"%PDF-1.4 synthetic".to_vec())],
        json!({}),
    );
    assert_eq!(prepared.unwrap_err().code, "storage");
    store.conn.commit_hook(None::<fn() -> bool>).unwrap();
    assert_eq!(snapshot(&store.conn), before);
    assert_eq!(
        store
            .conn
            .query_row("SELECT count(*) FROM prepare_operations", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn schema_five_to_six_is_atomic_and_preserves_history() {
    let (dir, store, _, id) = fixture();
    let before = snapshot(&store.conn);
    store
        .conn
        .execute_batch("DROP TABLE dsn_header_reports; PRAGMA user_version=5;")
        .unwrap();
    drop(store);
    let mut veto = Connection::open(dir.path().join("mail.sqlite3")).unwrap();
    veto.commit_hook(Some(|| true)).unwrap();
    assert!(schema::initialize(&mut veto).is_err());
    assert_eq!(
        veto.pragma_query_value(None, "user_version", |r| r.get::<_, i32>(0))
            .unwrap(),
        5
    );
    assert_eq!(snapshot(&veto), before);
    drop(veto);
    let migrated = Store::open(dir.path().to_path_buf()).unwrap();
    assert_eq!(snapshot(&migrated.conn), before);
    assert_eq!(migrated.status(&id).unwrap()["state"], "prepared");
}
