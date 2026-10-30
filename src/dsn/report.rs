//! Bounded raw-report interpretation and identity correlation, without I/O.
use super::envid;
use crate::{Result, err, invalid};
use serde_json::{Value, json};

fn headers(block: &str) -> Result<Vec<mailparse::MailHeader<'_>>> {
    let (headers, consumed) = mailparse::parse_headers(block.as_bytes()).map_err(|_| invalid())?;
    if consumed != block.len() || headers.is_empty() {
        return Err(invalid());
    }
    Ok(headers)
}
fn field(headers: &[mailparse::MailHeader<'_>], name: &str) -> Result<String> {
    let found: Vec<_> = headers
        .iter()
        .filter(|h| h.get_key_ref().eq_ignore_ascii_case(name))
        .collect();
    if found.len() != 1 {
        return Err(invalid());
    }
    let value = found[0].get_value();
    if value.trim().is_empty() || value.contains(['\r', '\n', '\0']) {
        return Err(invalid());
    }
    Ok(value)
}
fn single_address(headers: &[mailparse::MailHeader<'_>], name: &str) -> Result<String> {
    let value = field(headers, name)?;
    let list = mailparse::addrparse(&value).map_err(|_| invalid())?;
    if list.len() != 1 {
        return Err(invalid());
    }
    match list.first() {
        Some(mailparse::MailAddr::Single(single)) => Ok(single.addr.clone()),
        _ => Err(invalid()),
    }
}
pub(super) fn parse_report(
    raw: &[u8],
    package: &str,
    attempt: &str,
    recipient: &str,
) -> Result<Value> {
    parse_report_mode(raw, package, attempt, recipient, None)
}
pub(super) fn parse_report_mode(
    raw: &[u8],
    package: &str,
    attempt: &str,
    recipient: &str,
    original: Option<&crate::package::Manifest>,
) -> Result<Value> {
    let message = mailparse::parse_mail(raw).map_err(|_| invalid())?;
    field(&message.headers, "Content-Type")?;
    if !(2..=3).contains(&message.subparts.len())
        || !message.subparts[0].ctype.mimetype.starts_with("text/")
        || message.subparts[1].ctype.mimetype != "message/delivery-status"
    {
        return Err(invalid());
    }
    if message.ctype.mimetype != "multipart/report"
        || message.ctype.params.get("report-type").map(String::as_str) != Some("delivery-status")
    {
        return Err(invalid());
    }
    let parts: Vec<_> = message
        .subparts
        .iter()
        .filter(|p| p.ctype.mimetype == "message/delivery-status")
        .collect();
    if parts.len() != 1 {
        return Err(invalid());
    }
    field(&parts[0].headers, "Content-Type")?;
    let body = parts[0].get_body_raw().map_err(|_| invalid())?;
    let text = String::from_utf8(body)
        .map_err(|_| invalid())?
        .replace("\r\n", "\n");
    let blocks: Vec<_> = text.trim_end_matches('\n').split("\n\n").collect();
    if !(2..=3).contains(&blocks.len()) {
        return Err(invalid());
    }
    if let Some(help) = blocks.get(2) {
        // Some providers append a separate informational Help block. It must
        // never be mistaken for, or hide, a second recipient-status block.
        let extra = headers(help)?;
        if extra.len() != 1 {
            return Err(invalid());
        }
        field(&extra, "Help")?; // Informational text only; never interpret/fetch it.
    } // Exactly one selected recipient; no ambiguous aggregate reports.
    let common = headers(blocks[0])?;
    let target = headers(blocks[1])?;
    field(&common, "Reporting-MTA")?;
    if let Some(manifest) = original {
        if common
            .iter()
            .any(|h| h.get_key_ref().eq_ignore_ascii_case("Original-Envelope-Id"))
        {
            return Err(err(
                "dsn_binding",
                "Header correlation requires an absent envelope identifier",
                5,
            ));
        }
        let part = message.subparts.get(2).ok_or_else(invalid)?;
        field(&part.headers, "Content-Type")?;
        let original_raw = match part.ctype.mimetype.as_str() {
            "text/rfc822-headers" | "message/rfc822" => {
                part.get_body_raw().map_err(|_| invalid())?
            }
            _ => return Err(invalid()),
        };
        let (original_headers, _) =
            mailparse::parse_headers(&original_raw).map_err(|_| invalid())?;
        if field(&original_headers, "Message-ID")? != manifest.message_id
            || single_address(&original_headers, "From")? != manifest.sender
            || single_address(&original_headers, "To")? != manifest.recipient
        {
            return Err(err(
                "dsn_binding",
                "Original headers do not match the frozen package",
                5,
            ));
        }
    } else if field(&common, "Original-Envelope-Id")? != envid(package, attempt) {
        return Err(err(
            "dsn_binding",
            "Report envelope identifier does not match the selected attempt",
            5,
        ));
    }
    let final_recipient = field(&target, "Final-Recipient")?;
    let (kind, value) = final_recipient.split_once(';').ok_or_else(invalid)?;
    if !kind.trim().eq_ignore_ascii_case("rfc822") || value.trim() != recipient {
        return Err(err(
            "dsn_binding",
            "Report recipient does not match the frozen recipient",
            5,
        ));
    }
    let action = field(&target, "Action")?.to_ascii_lowercase();
    let code = field(&target, "Status")?;
    let numbers: Vec<_> = code.split('.').collect();
    if numbers.len() != 3
        || numbers.iter().any(|v| {
            v.is_empty()
                || (v.len() > 1 && v.starts_with('0'))
                || v.len() > 3
                || !v.bytes().all(|b| b.is_ascii_digit())
        })
    {
        return Err(invalid());
    }
    let valid = match action.as_str() {
        "delivered" | "relayed" | "expanded" => numbers[0] == "2",
        "delayed" => numbers[0] == "4",
        "failed" => matches!(numbers[0], "4" | "5"),
        _ => false,
    };
    if !valid {
        return Err(invalid());
    }
    let mut report = json!({"action":action,"status":code,"independently_verified":false,"retry_authorized":false});
    // Optional diagnostic metadata must not invalidate legacy archived reports.
    // Ambiguous/malformed prose remains only in raw evidence, never in status.
    if target
        .iter()
        .filter(|h| h.get_key_ref().eq_ignore_ascii_case("Diagnostic-Code"))
        .count()
        == 1
        && let Ok(diagnostic) = field(&target, "Diagnostic-Code")
        && let Some((kind, text)) = diagnostic.split_once(';')
        && kind.trim().eq_ignore_ascii_case("smtp")
        && let Some(code) = text
            .split_whitespace()
            .next()
            .filter(|v| v.len() == 3 && v.bytes().all(|b| b.is_ascii_digit()))
    {
        report["smtp_reply_code"] = json!(code);
    }

    Ok(report)
}
