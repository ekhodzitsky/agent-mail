use super::*;

#[cfg(test)]
mod parser_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    fn report() -> String {
        "Content-Type: multipart/report; report-type=delivery-status; boundary=x\r\n\r\n--x\r\nContent-Type: text/plain\r\n\r\nSynthetic\r\n--x\r\nContent-Type: message/delivery-status\r\n\r\nReporting-MTA: dns; example.org\r\nOriginal-Envelope-Id: package.attempt\r\n\r\nFinal-Recipient: rfc822; recipient@example.org\r\nAction: delivered\r\nStatus: 2.0.0\r\n\r\n--x--\r\n".into()
    }
    #[test]
    fn report_parser_rejects_ambiguous_or_malformed_reports() {
        assert!(
            parse_report(
                report().as_bytes(),
                "package",
                "attempt",
                "recipient@example.org"
            )
            .is_ok()
        );
        for raw in [
            report().replace("Status: 2.0.0","Status: 2.0.0\r\nStatus: 2.0.0"),
            report().replace("Original-Envelope-Id: package.attempt","Original-Envelope-Id: package.attempt\r\nOriginal-Envelope-Id: package.attempt"),
            report().replace("Action: delivered","Action: failed"),
            report().replace("Status: 2.0.0","Status: 2.x.0"),
            report().replace("message/delivery-status","message/global-delivery-status"),
            report().replace("Original-Envelope-Id: package.attempt\r\n", ""),
            report().replace("report-type=delivery-status", "report-type=disposition-notification"),
            report().replace("--x--", "--x\r\nContent-Type: message/delivery-status\r\n\r\nAction: delivered\r\n--x--"),
            report().replace("\r\n\r\n--x--", "\r\n\r\nFinal-Recipient: rfc822; other@example.org\r\nAction: delivered\r\nStatus: 2.0.0\r\n\r\n--x--"),
        ] {assert!(parse_report(raw.as_bytes(),"package","attempt","recipient@example.org").is_err());}
    }
    #[test]
    fn policy_requires_canonical_frozen_parameters() {
        let id = Uuid::new_v4().to_string();
        let mut p = DsnPolicy::new(DsnMode::Required, &id);
        assert!(p.valid(&id));
        p.ret = "FULL".into();
        assert!(!p.valid(&id));
        p.ret = "HDRS".into();
        p.notify = "NEVER".into();
        assert!(!p.valid(&id));
        p.notify = "SUCCESS,FAILURE,DELAY".into();
        p.mode = DsnMode::Off;
        assert!(!p.valid(&id));
    }
}

#[cfg(test)]
mod rfc_compatibility {
    #![allow(clippy::unwrap_used)]
    #[test]
    fn action_case_and_abandoned_temporary_failure_follow_rfc3464() {
        let raw=b"Content-Type: multipart/report; report-type=delivery-status; boundary=x\r\n\r\n--x\r\nContent-Type: text/plain\r\n\r\nSynthetic\r\n--x\r\nContent-Type: message/delivery-status\r\n\r\nReporting-MTA: dns; example.org\r\nOriginal-Envelope-Id: package.attempt\r\n\r\nFinal-Recipient: rfc822; recipient@example.org\r\nAction: FAILED\r\nStatus: 4.4.7\r\n\r\n--x--\r\n";
        let parsed =
            super::parse_report(raw, "package", "attempt", "recipient@example.org").unwrap();
        assert_eq!(parsed["action"], "failed");
        assert_eq!(parsed["status"], "4.4.7");
        let invalid = String::from_utf8(raw.to_vec())
            .unwrap()
            .replace("4.4.7", "4.04.7");
        assert!(
            super::parse_report(
                invalid.as_bytes(),
                "package",
                "attempt",
                "recipient@example.org"
            )
            .is_err()
        );
    }
}

#[cfg(test)]
mod header_correlation_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    fn raw(p: &Package) -> String {
        format!(
            "Content-Type: multipart/report; report-type=delivery-status; boundary=x\r\n\r\n--x\r\nContent-Type: text/plain\r\n\r\nSynthetic private message\r\n--x\r\nContent-Type: message/delivery-status\r\n\r\nReporting-MTA: dns; example.org\r\n\r\nFinal-Recipient: rfc822; {}\r\nAction: failed\r\nStatus: 5.0.0\r\nDiagnostic-Code: smtp; 550 synthetic private prose\r\n\r\n--x\r\nContent-Type: text/rfc822-headers\r\n\r\nMessage-ID: {}\r\nFrom: Synthetic <{}>\r\nTo: {}\r\n\r\n--x--\r\n",
            p.manifest.recipient, p.manifest.message_id, p.manifest.sender, p.manifest.recipient
        )
    }
    fn attempt(store: &Store, id: &str, attempt: &str) {
        let p = store.load(id).unwrap();
        store.conn.execute("INSERT INTO attempts VALUES(?1,?2,'2026-01-01T00:00:00+00:00','2026-01-01T00:00:01+00:00','accepted',?3,'250')",params![attempt,id,p.manifest.message_id]).unwrap();
    }
    #[test]
    fn explicit_header_import_deduplicates_preserves_attempts_and_blocks_resend() {
        let (dir, store, cfg, id) = crate::storage_tests::fixture();
        attempt(&store, &id, "attempt");
        let before = store.status(&id).unwrap();
        let source = raw(&store.load(&id).unwrap());
        let path = dir.path().join("bounce.eml");
        std::fs::write(&path, &source).unwrap();
        assert!(import(&store, &id, "attempt", &path).is_err());
        let first = import_mode(&store, &id, "attempt", &path, true).unwrap();
        assert_eq!(first["correlation"], "original_headers");
        assert_eq!(first["report"]["smtp_reply_code"], "550");
        assert!(!first.to_string().contains("private prose"));
        let again = import_mode(&store, &id, "attempt", &path, true).unwrap();
        assert_eq!(first["report_id"], again["report_id"]);
        assert_eq!(again["already_recorded"], true);
        let after = store.status(&id).unwrap();
        assert_eq!(after["attempts"], before["attempts"]);
        assert_eq!(after["state"], "accepted");
        assert_eq!(after["delivery_observation"]["outcome"], "failed_reported");
        assert_eq!(after["delivery_confirmed"], false);
        assert_eq!(
            crate::smtp::send(&store, &cfg, &id, false)
                .unwrap_err()
                .code,
            "send_blocked"
        );
        let saved = inspect(&store, first["report_id"].as_str().unwrap()).unwrap();
        assert_eq!(saved["record"]["sha256"], hash(source.as_bytes()));
        assert_eq!(std::fs::read(&path).unwrap(), source.as_bytes());
        assert!(
            store
                .conn
                .execute("DELETE FROM dsn_header_reports", [])
                .is_err()
        );
        assert!(
            store
                .conn
                .execute("UPDATE dsn_header_reports SET bytes=0", [])
                .is_err()
        );
    }
    #[test]
    fn header_binding_rejects_mismatches_duplicates_groups_and_envid_downgrade() {
        let (_dir, store, _, id) = crate::storage_tests::fixture();
        let p = store.load(&id).unwrap();
        let good = raw(&p);
        for bad in [
            good.replace(&p.manifest.message_id, "<other@example.org>"),
            good.replace(
                "From: Synthetic <sender@example.org>",
                "From: other@example.org",
            ),
            good.replace("To: recipient@example.org", "To: other@example.org"),
            good.replace(
                "To: recipient@example.org",
                "To: recipient@example.org, other@example.org",
            ),
            good.replace(
                "To: recipient@example.org",
                "To: group:recipient@example.org;",
            ),
            good.replace(
                "To: recipient@example.org",
                "To: recipient@example.org\r\nTo: recipient@example.org",
            ),
            good.replace(
                "Final-Recipient: rfc822; recipient@example.org",
                "Final-Recipient: rfc822; other@example.org",
            ),
            good.replace(
                "Reporting-MTA: dns; example.org",
                "Reporting-MTA: dns; example.org\r\nOriginal-Envelope-Id: wrong",
            ),
            good.replace("text/rfc822-headers", "text/plain"),
            good.replace("Message-ID:", "Ignored-ID:"),
        ] {
            assert!(
                parse_report_mode(
                    bad.as_bytes(),
                    &id,
                    "a",
                    &p.manifest.recipient,
                    Some(&p.manifest)
                )
                .is_err()
            );
        }
        assert!(
            parse_report_mode(
                good.replace("text/rfc822-headers", "message/rfc822")
                    .as_bytes(),
                &id,
                "a",
                &p.manifest.recipient,
                Some(&p.manifest)
            )
            .is_ok()
        );
    }
    #[test]
    fn informational_help_block_never_hides_another_recipient() {
        let (_dir, store, _, id) = crate::storage_tests::fixture();
        let p = store.load(&id).unwrap();
        let source = raw(&p);
        let with_help=source.replace("\r\n\r\n--x\r\nContent-Type: text/rfc822-headers", "\r\n\r\nHelp: https://example.org/help\r\n\r\n--x\r\nContent-Type: text/rfc822-headers");
        assert_ne!(source, with_help);
        for diagnostic in [
            "Diagnostic-Code:",
            "Diagnostic-Code: smtp; 550 ignored\r\nDiagnostic-Code: smtp; 550 duplicate",
        ] {
            let legacy = source.replace(
                "Diagnostic-Code: smtp; 550 synthetic private prose",
                diagnostic,
            );
            let parsed = parse_report_mode(
                legacy.as_bytes(),
                &id,
                "a",
                &p.manifest.recipient,
                Some(&p.manifest),
            )
            .unwrap();
            assert!(parsed.get("smtp_reply_code").is_none());
        }

        assert!(
            parse_report_mode(
                with_help
                    .replace(
                        "Help: https://example.org/help",
                        "Help: See provider support"
                    )
                    .as_bytes(),
                &id,
                "a",
                &p.manifest.recipient,
                Some(&p.manifest)
            )
            .is_ok()
        );
        assert!(
            parse_report_mode(
                with_help.as_bytes(),
                &id,
                "a",
                &p.manifest.recipient,
                Some(&p.manifest)
            )
            .is_ok()
        );
        for extra in [
            "Help: https://example.org/help\r\nFinal-Recipient: rfc822; other@example.org",
            "Help: https://example.org/help\r\nHelp: https://example.org/other",
            "Help:",
            "Final-Recipient: rfc822; other@example.org",
        ] {
            let bad = with_help.replace("Help: https://example.org/help", extra);
            assert!(
                parse_report_mode(
                    bad.as_bytes(),
                    &id,
                    "a",
                    &p.manifest.recipient,
                    Some(&p.manifest)
                )
                .is_err()
            );
        }
    }
    #[test]
    fn ambiguous_attempts_and_wrong_attempt_refuse_without_evidence() {
        let (dir, store, _, id) = crate::storage_tests::fixture();
        attempt(&store, &id, "a");
        let path = dir.path().join("report");
        std::fs::write(&path, raw(&store.load(&id).unwrap())).unwrap();
        assert!(import_mode(&store, &id, "other", &path, true).is_err());
        let (dir, store, _, id) = crate::storage_tests::fixture();
        let p = store.load(&id).unwrap();
        store.conn.execute("INSERT INTO attempts VALUES('rejected',?1,'2026-01-01T00:00:00+00:00','2026-01-01T00:00:01+00:00','rejected',?2,'550')",params![id,p.manifest.message_id]).unwrap();
        attempt(&store, &id, "a");
        let path = dir.path().join("report");
        std::fs::write(&path, raw(&p)).unwrap();
        assert!(import_mode(&store, &id, "a", &path, true).is_err());
        assert_eq!(store.status(&id).unwrap()["dsn"]["reports"], json!([]));
    }
    #[test]
    fn failed_commit_lock_and_tamper_leave_history_protected() {
        let (dir, store, _, id) = crate::storage_tests::fixture();
        attempt(&store, &id, "a");
        let path = dir.path().join("report");
        std::fs::write(&path, raw(&store.load(&id).unwrap())).unwrap();
        let lock = package_lock(&store, &id).unwrap();
        assert!(import_mode(&store, &id, "a", &path, true).is_err());
        drop(lock);
        store.conn.commit_hook(Some(|| true)).unwrap();
        assert!(import_mode(&store, &id, "a", &path, true).is_err());
        store.conn.commit_hook(None::<fn() -> bool>).unwrap();
        assert_eq!(store.status(&id).unwrap()["dsn"]["reports"], json!([]));
        import_mode(&store, &id, "a", &path, true).unwrap();
        store.conn.execute_batch("DROP TRIGGER dsn_header_reports_no_update; UPDATE dsn_header_reports SET raw=x'00';").unwrap();
        assert_eq!(store.status(&id).unwrap_err().code, "integrity");
    }
    #[test]
    fn deterministic_report_truncation_and_byte_mutations_do_not_escape_assertion_limits() {
        let (_dir, store, _, id) = crate::storage_tests::fixture();
        let mut p = store.load(&id).unwrap();
        p.manifest.message_id = "<synthetic-original@example.org>".into();
        let seed = raw(&p).into_bytes();
        let check = |data: &[u8]| {
            if let Ok(value) = parse_report_mode(
                data,
                "package",
                "attempt",
                &p.manifest.recipient,
                Some(&p.manifest),
            ) {
                assert_eq!(value["retry_authorized"], false);
                assert_eq!(value["independently_verified"], false);
                assert!(matches!(
                    value["action"].as_str(),
                    Some("delivered" | "relayed" | "expanded" | "delayed" | "failed")
                ));
            }
        };
        for end in 0..=seed.len() {
            check(&seed[..end]);
        }
        let mut state = 0x6d61696cu64;
        for _ in 0..512 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let mut bytes = seed.clone();
            let index = (state as usize) % bytes.len();
            bytes[index] ^= (state >> 32) as u8;
            check(&bytes);
        }
    }

    #[test]
    fn contradictory_reports_never_claim_delivery_or_authorize_retry() {
        let report = |id, action| json!({"report_id":id,"report":{"action":action}});
        let dsn = json!({"reports":[report("one","failed"),report("two","delivered")]});
        let summary = delivery_observation(&dsn).unwrap();
        assert_eq!(summary["outcome"], "conflicting_reports");
        assert_eq!(summary["retry_authorized"], false);
        assert_eq!(summary["independently_verified"], false);
        assert_eq!(
            delivery_observation(&json!({"reports":[report("one","relayed")]})).unwrap()["outcome"],
            "relayed_reported"
        );
    }
}
