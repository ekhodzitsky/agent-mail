//! Immutable package models, MIME preparation and exact export.
use crate::{
    Result,
    config::{Config, address},
    err,
    files::{MAX_PDF, MAX_TEXT},
    hash, invalid, now, storage,
    store::Store,
};
use lettre::{
    Message,
    message::{
        Attachment, Body, MultiPart, SinglePart,
        header::{ContentTransferEncoding, ContentType},
    },
};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{fs, io::Write, path::Path};
use uuid::Uuid;

pub(super) const MAX_OPERATION_KEY: usize = 128;

#[derive(Serialize, Deserialize)]
pub(super) struct Blob {
    pub(super) name: String,
    pub(super) sha256: String,
    pub(super) bytes: u64,
}
#[derive(Serialize, Deserialize)]
pub(super) struct HtmlBody {
    pub(super) content: String,
    pub(super) sha256: String,
    pub(super) bytes: u64,
}
#[derive(Serialize, Deserialize)]
pub(super) struct Manifest {
    pub(super) schema_version: u32,
    pub(super) id: String,
    pub(super) sender: String,
    pub(super) recipient: String,
    pub(super) subject: String,
    pub(super) body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) html: Option<HtmlBody>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) dsn: Option<crate::dsn::DsnPolicy>,
    pub(super) subject_sha256: String,
    pub(super) body_sha256: String,
    pub(super) subject_bytes: u64,
    pub(super) body_bytes: u64,
    pub(super) prepared_at: String,
    pub(super) message_id: String,
    pub(super) attachments: Vec<Blob>,
    pub(super) eml_sha256: String,
    pub(super) eml_bytes: u64,
    pub(super) metadata: Value,
}
pub(super) struct Package {
    pub(super) manifest: Manifest,
    pub(super) eml: Vec<u8>,
    pub(super) manifest_sha256: String,
}
pub(super) struct Content {
    pub body: String,
    pub html: Option<String>,
    pub dsn: Option<crate::dsn::DsnMode>,
    pub operation_key: Option<String>,
}
#[derive(Serialize)]
struct Input<'a> {
    request_schema_version: u32,
    sender: &'a str,
    recipient: &'a str,
    subject: &'a str,
    body: &'a str,
    html: Option<&'a str>,
    dsn: Option<&'a crate::dsn::DsnMode>,
    attachments: &'a [Blob],
    metadata: &'a Value,
}
impl Input<'_> {
    fn digest(&self) -> Result<String> {
        Ok(hash(&serde_json::to_vec(self).map_err(|_| storage())?))
    }
}
fn prepared_response(manifest: Manifest, digest: String, state: Value, reused: bool) -> Value {
    json!({"package_id":manifest.id,"state":state,"already_prepared":reused,
        "manifest_sha256":digest,"manifest":manifest})
}
pub(super) fn export_eml(store: &Store, id: &str, output: &Path) -> Result<Value> {
    // Verify every component before creating any output, even without SMTP config.
    let package = store.load(id)?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(output).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            err("output_exists", "Export destination already exists", 3)
        } else {
            storage()
        }
    })?;
    // On I/O failure a partial file can remain. Never delete/replace a path that
    // another process might now own; report failure and require a new destination.
    file.write_all(&package.eml).map_err(|_| storage())?;
    file.sync_all().map_err(|_| storage())?;
    Ok(
        json!({"package_id":id,"eml_sha256":package.manifest.eml_sha256,
        "eml_bytes":package.manifest.eml_bytes,"integrity":"verified","network_used":false}),
    )
}
pub(super) fn validate_text(subject: &str, body: &str) -> Result<()> {
    if subject.len() > 998
        || subject.trim().is_empty()
        || subject.contains(['\r', '\n', '\0'])
        || body.trim().is_empty()
        || body.contains('\0')
    {
        return Err(invalid());
    }
    Ok(())
}
pub(super) fn prepare(
    store: &mut Store,
    cfg: &Config,
    to: String,
    subject: String,
    body: String,
    files: Vec<(String, Vec<u8>)>,
    metadata: Value,
) -> Result<Value> {
    prepare_content(
        store,
        cfg,
        to,
        subject,
        Content {
            body,
            html: None,
            dsn: None,
            operation_key: None,
        },
        files,
        metadata,
    )
}
pub(super) fn prepare_content(
    store: &mut Store,
    cfg: &Config,
    to: String,
    subject: String,
    content: Content,
    files: Vec<(String, Vec<u8>)>,
    metadata: Value,
) -> Result<Value> {
    let Content {
        body,
        html,
        dsn,
        operation_key,
    } = content;
    address(&to)?;
    validate_text(&subject, &body)?;
    if files.is_empty() || files.len() > 5 || body.len() as u64 > MAX_TEXT {
        return Err(invalid());
    }
    let attachments: Vec<_> = files
        .iter()
        .map(|(name, bytes)| {
            if name.is_empty()
                || name.contains(['\r', '\n', '\0', '/', '\\'])
                || !bytes.starts_with(b"%PDF-")
                || bytes.len() as u64 > MAX_PDF
            {
                return Err(invalid());
            }
            Ok(Blob {
                name: name.clone(),
                sha256: hash(bytes),
                bytes: bytes.len() as u64,
            })
        })
        .collect::<Result<_>>()?;
    if operation_key.as_ref().is_some_and(|key| {
        key.is_empty()
            || key.len() > MAX_OPERATION_KEY
            || !key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-._:/".contains(&b))
    }) {
        return Err(invalid());
    }
    let dsn = dsn.filter(|mode| *mode != crate::dsn::DsnMode::Off);
    let input_digest = Input {
        request_schema_version: 1,
        sender: &cfg.sender,
        recipient: &to,
        subject: &subject,
        body: &body,
        html: html.as_deref(),
        dsn: dsn.as_ref(),
        attachments: &attachments,
        metadata: &metadata,
    }
    .digest()?;
    let key_digest = operation_key.as_ref().map(|key| hash(key.as_bytes()));
    // Serialize lookup + package/key publication, including concurrent processes.
    let tx = store
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| storage())?;
    if let Some(key) = &key_digest {
        let prior: Option<(String, String)> = tx
            .query_row(
                "SELECT input_sha256,package_id FROM prepare_operations WHERE key_sha256=?1",
                [key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|_| storage())?;
        if let Some((digest, id)) = prior {
            if digest != input_digest {
                return Err(err(
                    "operation_conflict",
                    "Operation key is already bound to different input",
                    5,
                ));
            }
            tx.commit().map_err(|_| storage())?;
            let package = store.load(&id)?;
            let m = &package.manifest;
            let actual = Input {
                request_schema_version: 1,
                sender: &m.sender,
                recipient: &m.recipient,
                subject: &m.subject,
                body: &m.body,
                html: m.html.as_ref().map(|h| h.content.as_str()),
                dsn: m.dsn.as_ref().map(|p| &p.mode),
                attachments: &m.attachments,
                metadata: &m.metadata,
            }
            .digest()?;
            if actual != digest {
                return Err(crate::integrity());
            }
            let state = store.status(&id)?["state"].clone();
            return Ok(prepared_response(
                package.manifest,
                package.manifest_sha256,
                state,
                true,
            ));
        }
    }
    let id = Uuid::new_v4().to_string();
    let dsn = dsn.map(|mode| crate::dsn::DsnPolicy::new(mode, &id));
    if dsn.is_some() && (!to.is_ascii() || !cfg.sender.is_ascii()) {
        return Err(invalid());
    }
    let message_id = format!(
        "<{id}@{}>",
        cfg.sender.rsplit('@').next().ok_or_else(invalid)?
    );
    if html
        .as_ref()
        .is_some_and(|h| h.trim().is_empty() || h.contains('\0') || h.len() as u64 > MAX_TEXT)
    {
        return Err(invalid());
    }
    let plain = SinglePart::builder().header(ContentType::TEXT_PLAIN).body(
        Body::new_with_encoding(body.as_bytes().to_vec(), ContentTransferEncoding::Base64)
            .map_err(|_| invalid())?,
    );
    let mut multipart = if let Some(ref html) = html {
        let html_part = SinglePart::builder().header(ContentType::TEXT_HTML).body(
            Body::new_with_encoding(html.as_bytes().to_vec(), ContentTransferEncoding::Base64)
                .map_err(|_| invalid())?,
        );
        MultiPart::mixed().multipart(
            MultiPart::alternative()
                .singlepart(plain)
                .singlepart(html_part),
        )
    } else {
        MultiPart::mixed().singlepart(plain)
    };
    for (name, bytes) in &files {
        multipart = multipart.singlepart(
            Attachment::new(name.clone()).body(
                Body::new_with_encoding(bytes.clone(), ContentTransferEncoding::Base64)
                    .map_err(|_| invalid())?,
                ContentType::parse("application/pdf").map_err(|_| invalid())?,
            ),
        );
    }
    let message = Message::builder()
        .from(cfg.sender.parse().map_err(|_| invalid())?)
        .to(to.parse().map_err(|_| invalid())?)
        .subject(&subject)
        .message_id(Some(message_id.clone()))
        .multipart(multipart)
        .map_err(|_| invalid())?;
    let eml = message.formatted();
    let manifest = Manifest {
        schema_version: if dsn.is_some() {
            3
        } else if html.is_some() {
            2
        } else {
            1
        },
        dsn,
        html: html.map(|content| HtmlBody {
            sha256: hash(content.as_bytes()),
            bytes: content.len() as u64,
            content,
        }),
        id: id.clone(),
        sender: cfg.sender.clone(),
        recipient: to,
        subject_sha256: hash(subject.as_bytes()),
        body_sha256: hash(body.as_bytes()),
        subject_bytes: subject.len() as u64,
        body_bytes: body.len() as u64,
        subject,
        body,
        prepared_at: now(),
        message_id,
        attachments,
        eml_sha256: hash(&eml),
        eml_bytes: eml.len() as u64,
        metadata,
    };
    let raw = serde_json::to_vec(&manifest).map_err(|_| storage())?;
    let digest = hash(&raw);
    tx.execute(
        "INSERT INTO packages VALUES(?1,?2,?3,?4)",
        params![id, raw, digest, eml],
    )
    .map_err(|_| storage())?;
    for (i, (_, bytes)) in files.iter().enumerate() {
        tx.execute(
            "INSERT INTO attachments VALUES(?1,?2,?3)",
            params![id, i as i64, bytes],
        )
        .map_err(|_| storage())?;
    }
    if let Some(key) = key_digest {
        tx.execute(
            "INSERT INTO prepare_operations VALUES(?1,?2,?3,?4)",
            params![key, input_digest, id, manifest.prepared_at],
        )
        .map_err(|_| storage())?;
    }
    tx.commit().map_err(|_| storage())?;
    Ok(prepared_response(
        manifest,
        digest,
        json!("prepared"),
        false,
    ))
}
