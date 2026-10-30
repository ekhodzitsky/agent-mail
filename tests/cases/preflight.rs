use super::*;

#[test]
fn preflight_distinguishes_configuration_smtp_and_environment_without_state() {
    let f = Fixture::new();
    let config = f.dir.path().join("config.toml");
    let home = f.dir.path().join("not-opened");
    let check = |value: Option<&str>| {
        let mut c = Command::new(binary());
        c.args([
            "--home",
            home.to_str().unwrap(),
            "--config",
            config.to_str().unwrap(),
            "preflight",
            "--json",
        ]);
        c.env_remove("SMTP_SYNTHETIC_PREFLIGHT");
        if let Some(value) = value {
            c.env("SMTP_SYNTHETIC_PREFLIGHT", value);
        }
        let (code, envelope) = decode(c.output().unwrap());
        assert_eq!(code, 0);
        assert!(envelope["ok"].as_bool().unwrap());
        let result = envelope["result"].clone();
        assert_eq!(result["storage_opened"], false);
        assert_eq!(result["network_used"], false);
        assert!(!home.exists());
        result
    };
    assert_eq!(check(None)["checks"]["smtp"], "not_configured");
    fs::remove_file(&config).unwrap();
    assert_eq!(check(None)["checks"]["configuration"], "missing");
    fs::write(&config, "synthetic private malformed data").unwrap();
    assert_eq!(check(None)["checks"]["configuration"], "invalid");
    fs::write(&config,"sender='sender@example.org'\n[smtp]\nhost='smtp.example.invalid'\nport=465\nsecurity='tls'\nusername='login@example.org'\npassword_env='SMTP_SYNTHETIC_PREFLIGHT'\n").unwrap();
    let missing = check(None);
    assert_eq!(missing["checks"]["smtp"], "ready");
    assert_eq!(missing["checks"]["credential_environment"], "unavailable");
    assert_eq!(missing["ready"], false);
    assert_eq!(check(Some(""))["ready"], false);
    let ready = check(Some("synthetic-private-credential-do-not-echo"));
    assert_eq!(ready["ready"], true);
    assert!(!ready.to_string().contains("synthetic-private-credential"));
    assert!(!ready.to_string().contains("login@example.org"));
    fs::write(&config,"sender='sender@example.org'\n[smtp]\nhost='smtp.example.invalid'\nport=465\nsecurity='insecure'\n").unwrap();
    assert_eq!(check(None)["checks"]["smtp"], "invalid");
}

#[test]
fn preflight_explicit_config_needs_no_home_or_database() {
    let f = Fixture::new();
    let config = f.dir.path().join("config.toml");
    let bytes = fs::read(&config).unwrap();
    let output = Command::new(binary())
        .args(["--config", config.to_str().unwrap(), "preflight", "--json"])
        .env_remove("HOME")
        .env_remove("AGENT_MAIL_HOME")
        .output()
        .unwrap();
    let (code, result) = decode(output);
    assert_eq!(code, 0);
    assert_eq!(result["result"]["storage_opened"], false);
    assert_eq!(fs::read(config).unwrap(), bytes);
    assert!(!f.dir.path().join("store").exists());
}
