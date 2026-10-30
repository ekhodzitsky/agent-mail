//! SQLite storage and verified package/status reads.
use crate::{
    Result, err,
    files::private_file,
    hash, integrity,
    package::{Manifest, Package},
    reconciliation, schema, storage,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde_json::{Value, json};
use std::{fs, path::PathBuf, time::Duration};

pub(super) struct Store {
    pub(super) conn: Connection,
    pub(super) home: PathBuf,
}
impl Store {
    pub(super) fn open(home: PathBuf) -> Result<Self> {
        if home.is_relative() && home.as_os_str().as_encoded_bytes().starts_with(b"file:") {
            // Older versions interpreted this ambiguously. Do not silently open
            // a fresh journal where an affected installation expects old state.
            return Err(err(
                "configuration",
                "Use an absolute filesystem path for a state directory beginning with file:",
                3,
            ));
        }
        match fs::symlink_metadata(&home) {
            Ok(meta) if !meta.file_type().is_dir() => return Err(storage()),
            Ok(_) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(_) => return Err(storage()),
        }
        fs::create_dir_all(&home).map_err(|_| storage())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&home, fs::Permissions::from_mode(0o700)).map_err(|_| storage())?;
        }
        let path = home.join("mail.sqlite3");
        let file = private_file(&path)?;
        drop(file);
        // Resolve the checked file to an absolute filesystem path. A relative
        // `file:` prefix is a SQLite URI even without OPEN_URI when the bundled
        // library enables URI handling globally. Keep that flag disabled too.
        let path = fs::canonicalize(path).map_err(|_| storage())?;
        let flags = OpenFlags::default() & !OpenFlags::SQLITE_OPEN_URI;
        let mut conn = Connection::open_with_flags(path, flags).map_err(|_| storage())?;
        conn.busy_timeout(Duration::from_secs(5))
            .map_err(|_| storage())?;
        schema::initialize(&mut conn)?;
        Ok(Self { conn, home })
    }
    pub(super) fn load(&self, id: &str) -> Result<Package> {
        let row: Option<(Vec<u8>, String, Vec<u8>)> = self
            .conn
            .query_row(
                "SELECT manifest,digest,eml FROM packages WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(|_| storage())?;
        let (raw, digest, eml) = row.ok_or_else(|| err("not_found", "Package not found", 3))?;
        if hash(&raw) != digest {
            return Err(integrity());
        }
        let m: Manifest = serde_json::from_slice(&raw).map_err(|_| integrity())?;
        let dsn_valid = match (&m.dsn, m.schema_version) {
            (None, 1 | 2) => true,
            (Some(policy), 3) => {
                policy.valid(&m.id) && m.sender.is_ascii() && m.recipient.is_ascii()
            }
            _ => false,
        };
        let valid_html = match (&m.html, m.schema_version) {
            (None, 1 | 3) => true,
            (Some(html), 2 | 3) => {
                html.bytes == html.content.len() as u64
                    && html.sha256 == hash(html.content.as_bytes())
                    && !html.content.trim().is_empty()
                    && !html.content.contains('\0')
            }
            _ => false,
        };
        if !dsn_valid
            || !valid_html
            || m.id != id
            || hash(&eml) != m.eml_sha256
            || eml.len() as u64 != m.eml_bytes
            || hash(m.body.as_bytes()) != m.body_sha256
            || hash(m.subject.as_bytes()) != m.subject_sha256
            || m.body.len() as u64 != m.body_bytes
            || m.subject.len() as u64 != m.subject_bytes
        {
            return Err(integrity());
        }
        let mut stmt = self
            .conn
            .prepare("SELECT idx,content FROM attachments WHERE package_id=?1 ORDER BY idx")
            .map_err(|_| storage())?;
        let rows = stmt
            .query_map([id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?)))
            .map_err(|_| storage())?;
        let mut count = 0;
        for row in rows {
            let (idx, content) = row.map_err(|_| storage())?;
            let meta = m.attachments.get(count).ok_or_else(integrity)?;
            if idx != count as i64
                || hash(&content) != meta.sha256
                || content.len() as u64 != meta.bytes
            {
                return Err(integrity());
            }
            count += 1;
        }
        if count != m.attachments.len() || count == 0 {
            return Err(integrity());
        }
        Ok(Package {
            manifest: m,
            eml,
            manifest_sha256: digest,
        })
    }
    pub(super) fn status(&self, id: &str) -> Result<Value> {
        self.load(id)?;
        let mut stmt=self.conn.prepare("SELECT id,started_at,finished_at,outcome,message_id,smtp_code FROM attempts WHERE package_id=?1 ORDER BY rowid").map_err(|_| storage())?;
        let rows=stmt.query_map([id],|r|Ok(json!({"attempt_id":r.get::<_,String>(0)?,"started_at":r.get::<_,String>(1)?,"finished_at":r.get::<_,Option<String>>(2)?,"outcome":r.get::<_,String>(3)?,"message_id":r.get::<_,String>(4)?,"server_response":{"code":r.get::<_,Option<String>>(5)?,"text_retained":false}}))).map_err(|_|storage())?;
        let attempts = rows
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| storage())?;
        let state = attempts
            .last()
            .map(|a| a["outcome"].clone())
            .unwrap_or(json!("prepared"));
        let records = reconciliation::list(self, id)?;
        let reported = records
            .iter()
            .any(|r| r["asserted_conclusion"] == "received");
        let dsn = crate::dsn::status(self, id)?;
        let delivery = crate::dsn::delivery_observation(&dsn)?;
        Ok(
            json!({"package_id":id,"state":state,"delivery_confirmed":false,"delivery_observation":delivery,"recipient_reported_received":reported,"attempts":attempts,"reconciliations":records,"dsn":dsn}),
        )
    }
}
