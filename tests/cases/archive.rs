use super::*;

#[test]
fn listing_pages_preserve_order_legacy_shape_and_selected_verification() {
    let f = Fixture::new();
    let ids: Vec<_> = (0..5).map(|_| f.prepare()).collect();
    let legacy = f.call(&["list"]).1["result"].clone();
    assert_eq!(legacy.as_object().unwrap().len(), 1);
    assert_eq!(legacy["packages"].as_array().unwrap().len(), 5);
    let mut all = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let mut args = vec!["list", "--limit", "2"];
        if let Some(ref id) = after {
            args.extend(["--after", id]);
        }
        let (code, result) = f.call(&args);
        assert_eq!(code, 0);
        all.extend(result["result"]["packages"].as_array().unwrap().clone());
        after = result["result"]["pagination"]["next_after"]
            .as_str()
            .map(str::to_owned);
        if after.is_none() {
            break;
        }
    }
    assert_eq!(Value::Array(all), legacy["packages"]);
    assert_eq!(f.call(&["list", "--summary"]).0, 2);
    assert_eq!(f.call(&["list", "--limit", "101"]).0, 2);
    assert_eq!(f.call(&["list", "--limit", "0"]).0, 2);
    assert_eq!(f.call(&["list", "--limit", "1", "--after", "missing"]).0, 3);
    let added = f.prepare();
    let (code, page) = f.call(&["list", "--limit", "100", "--after", &ids[1], "--summary"]);
    assert_eq!(code, 0);
    let selected: Vec<_> = page["result"]["packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["package_id"].as_str().unwrap())
        .collect();
    assert_eq!(
        selected,
        vec![
            ids[2].as_str(),
            ids[3].as_str(),
            ids[4].as_str(),
            added.as_str()
        ]
    );
    assert_eq!(
        f.call(&["list", "--limit", "1", "--after", &added]).1["result"]["packages"],
        json!([])
    );
    // Corruption outside the page must not read all attachments; selected full
    // status still refuses, whereas ID-only summaries explicitly skip integrity.
    let db = rusqlite::Connection::open(f.dir.path().join("store/mail.sqlite3")).unwrap();
    db.execute_batch("DROP TRIGGER attachments_no_update")
        .unwrap();
    db.execute(
        "UPDATE attachments SET content=x'00' WHERE package_id=?1",
        [&ids[4]],
    )
    .unwrap();
    db.execute_batch("CREATE TRIGGER attachments_no_update BEFORE UPDATE ON attachments BEGIN SELECT RAISE(ABORT,'immutable'); END").unwrap();
    drop(db);
    assert_eq!(f.call(&["list", "--limit", "1"]).0, 0);
    assert_eq!(f.call(&["list", "--limit", "1", "--after", &ids[3]]).0, 4);
    let (code, result) = f.call(&["list", "--limit", "1", "--after", &ids[3], "--summary"]);
    assert_eq!(code, 0);
    assert_eq!(
        result["result"]["packages"][0],
        json!({"package_id":ids[4],"integrity":"not_checked"})
    );
}

#[test]
fn backup_preserves_attempts_verifies_without_home_and_never_claims_freshness() {
    let f = Fixture::new();
    let unknown = f.prepare();
    let accepted = f.prepare();
    let db = rusqlite::Connection::open(f.dir.path().join("store/mail.sqlite3")).unwrap();
    for (id, state) in [(&unknown, "unknown"), (&accepted, "accepted")] {
        let message = f.call(&["inspect", id]).1["result"]["manifest"]["message_id"]
            .as_str()
            .unwrap()
            .to_owned();
        db.execute(
            "INSERT INTO attempts VALUES(?1,?2,'2026-01-01T00:00:00+00:00',?3,?4,?5,NULL)",
            rusqlite::params![
                state,
                id,
                if state == "accepted" {
                    Some("2026-01-01T00:01:00+00:00")
                } else {
                    None
                },
                state,
                message
            ],
        )
        .unwrap();
    }
    drop(db);
    let expected: Vec<_> = [&unknown, &accepted]
        .iter()
        .map(|id| f.call(&["status", id]).1["result"].clone())
        .collect();
    let snapshot = f.dir.path().join("file:snapshot");
    let (code, result) = decode(
        f.cmd(&["backup", "--output", "file:snapshot"])
            .current_dir(f.dir.path())
            .output()
            .unwrap(),
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["result"]["freshness"], "not_established");
    assert_eq!(result["result"]["retry_authorized"], false);
    assert!(!snapshot.join("config.toml").exists());
    assert_eq!(
        f.call(&["backup", "--output", snapshot.to_str().unwrap()])
            .0,
        7
    );
    let before = fs::read(snapshot.join("mail.sqlite3")).unwrap();
    let absent = f.dir.path().join("must-not-create");
    let output = Command::new(binary())
        .args([
            "--home",
            absent.to_str().unwrap(),
            "verify-backup",
            snapshot.to_str().unwrap(),
            "--json",
        ])
        .env_remove("HOME")
        .output()
        .unwrap();
    let (code, result) = decode(output);
    assert_eq!(code, 0, "{result}");
    assert!(!absent.exists());
    let output = Command::new(binary())
        .args([
            "--home",
            absent.to_str().unwrap(),
            "backup",
            "--output",
            f.dir.path().join("must-not-snapshot").to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(decode(output).0, 7);
    assert!(!absent.exists());
    assert_eq!(fs::read(snapshot.join("mail.sqlite3")).unwrap(), before);
    fs::write(f.dir.path().join("config.toml"),"sender='sender@example.org'\n[smtp]\nhost='smtp.example.invalid'\nport=465\nsecurity='tls'\n").unwrap();
    for (id, status) in [&unknown, &accepted].iter().zip(expected) {
        let output = Command::new(binary())
            .args(["--home", snapshot.to_str().unwrap(), "--json", "status", id])
            .output()
            .unwrap();
        assert_eq!(decode(output).1["result"], status);
        let output = Command::new(binary())
            .args([
                "--home",
                snapshot.to_str().unwrap(),
                "--config",
                f.dir.path().join("config.toml").to_str().unwrap(),
                "--json",
                "send",
                id,
            ])
            .output()
            .unwrap();
        assert_eq!(decode(output).0, 5);
    }
}

#[test]
fn incomplete_or_tampered_backup_refuses_and_old_valid_copy_remains_stale() {
    let f = Fixture::new();
    let id = f.prepare();
    let snapshot = f.dir.path().join("snapshot");
    assert_eq!(
        f.call(&["backup", "--output", snapshot.to_str().unwrap()])
            .0,
        0
    );
    // A later attempt does not retroactively update an old backup. Verification
    // reports integrity, explicitly not currency or permission to submit.
    let db = rusqlite::Connection::open(f.dir.path().join("store/mail.sqlite3")).unwrap();
    let message = f.call(&["inspect", &id]).1["result"]["manifest"]["message_id"]
        .as_str()
        .unwrap()
        .to_owned();
    db.execute("INSERT INTO attempts VALUES('later',?1,'2026-01-01T00:00:00+00:00',NULL,'unknown',?2,NULL)",rusqlite::params![id,message]).unwrap();
    drop(db);
    let (code, result) = f.call(&["verify-backup", snapshot.to_str().unwrap()]);
    assert_eq!(code, 0);
    assert_eq!(result["result"]["freshness"], "not_established");
    let manifest = fs::read(snapshot.join("backup.json")).unwrap();
    fs::remove_file(snapshot.join("backup.json")).unwrap();
    assert_ne!(f.call(&["verify-backup", snapshot.to_str().unwrap()]).0, 0);
    fs::write(snapshot.join("backup.json"), manifest).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            snapshot.join("backup.json"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
    }
    let mut bytes = fs::read(snapshot.join("mail.sqlite3")).unwrap();
    bytes[100] ^= 1;
    fs::write(snapshot.join("mail.sqlite3"), bytes).unwrap();
    assert_eq!(f.call(&["verify-backup", snapshot.to_str().unwrap()]).0, 4);
    assert_eq!(f.call(&["status", &id]).1["result"]["state"], "unknown");
}

#[test]
fn backup_refuses_future_source_without_migration_or_output() {
    let f = Fixture::new();
    f.prepare();
    let path = f.dir.path().join("store/mail.sqlite3");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch("PRAGMA user_version=7").unwrap();
    conn.close().unwrap();
    let before = fs::read(&path).unwrap();
    let output = f.dir.path().join("snapshot");
    assert_eq!(
        f.call(&["backup", "--output", output.to_str().unwrap()]).0,
        9
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(!output.exists());
}
