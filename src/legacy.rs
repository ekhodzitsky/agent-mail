//! Read-only adapter for legacy application archive v1.
use crate::{
    Result,
    config::Config,
    err,
    files::{MAX_PDF, MAX_TEXT, read_bounded, text_file},
    hash, integrity, invalid,
    package::prepare,
    store::Store,
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn legacy_file(
    source: &Path,
    manifest: &Value,
    key: &str,
    expected: &str,
) -> Result<(String, Vec<u8>)> {
    let item = &manifest[key];
    if item["file"].as_str() != Some(expected) {
        return Err(invalid());
    }
    let path = source.join(expected);
    if fs::symlink_metadata(&path)
        .map_err(|_| invalid())?
        .file_type()
        .is_symlink()
    {
        return Err(invalid());
    }
    let bytes = read_bounded(&path, if key == "pdf" { MAX_PDF } else { MAX_TEXT })?;
    if item["sha256"].as_str() != Some(hash(&bytes).as_str())
        || item["bytes"].as_u64() != Some(bytes.len() as u64)
    {
        return Err(integrity());
    }
    let name = item["source_filename"]
        .as_str()
        .ok_or_else(invalid)?
        .to_string();
    Ok((name, bytes))
}
pub(super) fn import(
    store: &mut Store,
    cfg: &Config,
    source: &Path,
    to: String,
    subject_file: Option<PathBuf>,
) -> Result<Value> {
    let receipt_present = match source.join("receipt.json").symlink_metadata() {
        Ok(_) => true,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(_) => return Err(invalid()),
    };
    if fs::symlink_metadata(source)
        .map_err(|_| invalid())?
        .file_type()
        .is_symlink()
        || receipt_present
    {
        return Err(err(
            "import_blocked",
            "Only unconfirmed prepared source packages may be imported",
            5,
        ));
    }
    let path = source.join("manifest.json");
    if fs::symlink_metadata(&path)
        .map_err(|_| invalid())?
        .file_type()
        .is_symlink()
    {
        return Err(invalid());
    }
    let raw = read_bounded(&path, MAX_TEXT)?;
    let m: Value = serde_json::from_slice(&raw).map_err(|_| invalid())?;
    let id = m["id"].as_str().ok_or_else(invalid)?;
    if m["schema_version"] != 1
        || source.file_name().and_then(|n| n.to_str()) != Some(id)
        || !id.starts_with('A')
        || id.len() < 5
        || !id[1..].bytes().all(|c| c.is_ascii_digit())
    {
        return Err(invalid());
    }
    let pdf = legacy_file(source, &m, "pdf", "resume.pdf")?;
    let (_, body) = legacy_file(source, &m, "letter", "cover-letter.txt")?;
    let subject = if m.get("subject").is_some() {
        if subject_file.is_some() {
            return Err(invalid());
        }
        String::from_utf8(legacy_file(source, &m, "subject", "subject.txt")?.1)
            .map_err(|_| invalid())?
    } else {
        text_file(&subject_file.ok_or_else(invalid)?)?
    };
    prepare(
        store,
        cfg,
        to,
        subject,
        String::from_utf8(body).map_err(|_| invalid())?,
        vec![pdf],
        json!({"source_id":id,"source_manifest_sha256":hash(&raw),"company_ref":m["company"],"vacancy_ref":m["role"]}),
    )
}
