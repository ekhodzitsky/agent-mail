use super::*;

fn command(root: &std::path::Path, args: &[&str]) -> Command {
    let mut c = Command::new(binary());
    c.env("HOME", root)
        .env_remove("AGENT_MAIL_HOME")
        .env_remove("AGENT_MAILER_HOME")
        .arg("--json")
        .args(args);
    c
}

#[test]
fn default_home_keeps_existing_history_and_blocks_resends() {
    for outcome in ["accepted", "unknown"] {
        let f = Fixture::new();
        let id = f.prepare();
        let (_, inspected) = f.call(&["inspect", &id]);
        let message_id = inspected["result"]["manifest"]["message_id"]
            .as_str()
            .unwrap();
        let old = f.dir.path().join(".local/share/agent-mailer");
        fs::create_dir_all(old.parent().unwrap()).unwrap();
        fs::rename(f.dir.path().join("store"), &old).unwrap();
        fs::copy(f.dir.path().join("config.toml"), old.join("config.toml")).unwrap();
        let db = rusqlite::Connection::open(old.join("mail.sqlite3")).unwrap();
        db.execute("INSERT INTO attempts VALUES('synthetic',?1,'2026-01-01T00:00:00+00:00',NULL,?2,?3,NULL)", rusqlite::params![id, outcome, message_id]).unwrap();
        drop(db);
        let (code, status) = decode(command(f.dir.path(), &["status", &id]).output().unwrap());
        assert_eq!(code, 0, "{status}");
        assert_eq!(status["result"]["state"], outcome);
        assert_eq!(
            decode(
                command(f.dir.path(), &["send", &id, "--dry-run"])
                    .output()
                    .unwrap()
            )
            .0,
            5
        );
        assert!(!f.dir.path().join(".local/share/agent-mail").exists());
        let (_, after) = decode(command(f.dir.path(), &["inspect", &id]).output().unwrap());
        assert_eq!(inspected["result"], after["result"]);
    }
}

#[test]
fn home_selection_rejects_ambiguity_and_respects_explicit_selection() {
    let f = Fixture::new();
    let root = f.dir.path();
    for name in ["agent-mail", "agent-mailer"] {
        fs::create_dir_all(root.join(".local/share").join(name)).unwrap();
    }
    let (code, error) = decode(command(root, &["doctor"]).output().unwrap());
    assert_eq!(code, 3);
    assert_eq!(error["error"]["code"], "home_ambiguous");
    for name in ["agent-mail", "agent-mailer"] {
        assert!(
            !root
                .join(".local/share")
                .join(name)
                .join("mail.sqlite3")
                .exists()
        );
    }
    let explicit = root.join("chosen");
    let mut c = command(root, &["list", "--home", explicit.to_str().unwrap()]);
    c.env("AGENT_MAIL_HOME", "one")
        .env("AGENT_MAILER_HOME", "two");
    assert_eq!(decode(c.output().unwrap()).0, 0);
    let mut c = command(root, &["doctor"]);
    c.env("AGENT_MAIL_HOME", "one")
        .env("AGENT_MAILER_HOME", "two");
    assert_eq!(
        decode(c.output().unwrap()).1["error"]["code"],
        "home_ambiguous"
    );
}

#[test]
fn fresh_default_and_both_home_environment_names_select_the_expected_store() {
    let f = Fixture::new();
    let root = f.dir.path();
    assert_eq!(decode(command(root, &["list"]).output().unwrap()).0, 0);
    assert!(root.join(".local/share/agent-mail/mail.sqlite3").exists());
    assert!(!root.join(".local/share/agent-mailer").exists());
    for name in ["AGENT_MAIL_HOME", "AGENT_MAILER_HOME"] {
        let chosen = root.join(name);
        let mut c = command(root, &["list"]);
        c.env(name, &chosen);
        assert_eq!(decode(c.output().unwrap()).0, 0);
        assert!(chosen.join("mail.sqlite3").exists());
    }
}
