# Legacy application archive adapter

`agent-mailer import /absolute/path/A0001 --to recipient@example.org --json` reads one explicitly selected prepared archive. It never scans an archive root, changes source files or writes Markdown. It produces a **new** local package ID with metadata.source_id and metadata.source_manifest_sha256, preserving company/role as optional opaque references.

The accepted external archive format is schema 1 below; it is not the core package manifest. All published fixtures/tests are synthetic, and only one explicitly selected source directory is read.

Required source files: `manifest.json`, `resume.pdf`, `cover-letter.txt`. Manifest PDF/letter entries must contain the canonical filename, original source_filename, SHA-256 and byte count. Schema must be 1 and manifest.id must match the `A` plus at least four digits directory name. Hashes and sizes are checked before preparation. A receipt.json of any kind blocks import, because this command imports only unconfirmed preparations.

If manifest.subject exists, `subject.txt` must match its hash/size. It is used unchanged; overriding it is rejected. If the source has no subject, pass `--subject-file PATH` explicitly. The source never supplies the sender: current local configuration supplies it, and `--to` explicitly supplies the recipient; both become frozen in the new package and must be reviewed before sending. An absent letter is unsupported in this first version, even if a legacy no-letter reason exists.

Package directories, manifest and referenced files cannot themselves be symbolic links. Only canonical relative filenames are read from the package; arbitrary manifest paths are rejected. Use a trusted, quiescent source directory: this is not a sandbox against another process rewriting path ancestors concurrently. Source hashes detect changed files but cannot prove historical authenticity. Preparing/importing the same materials twice yields separate packages and is not cross-package deduplication.

For later Markdown integration, consume schema-versioned `status` output in a separately authorized adapter. The base CLI contains no Markdown-editing command and makes no assumptions about the owner's repository layout beyond this explicit read-only import format.

## Source manifest shape

```json
{
  "schema_version": 1,
  "id": "A0001",
  "company": "demo-company",
  "role": "demo-role",
  "pdf": {"file":"resume.pdf","source_filename":"resume.pdf","sha256":"SHA256_OF_EXACT_PDF","bytes":123},
  "letter": {"file":"cover-letter.txt","source_filename":"letter.txt","sha256":"SHA256_OF_EXACT_LETTER","bytes":45},
  "subject": {"file":"subject.txt","source_filename":"subject.txt","sha256":"SHA256_OF_EXACT_SUBJECT","bytes":21}
}
```

This is a structural example, not a valid hashed fixture: substitute actual byte counts/hashes. The directory must be named A0001 for that ID. Subject is optional only when --subject-file supplies it explicitly; PDF/letter are required. Input filenames are fixed, not arbitrary paths. The PDF's source_filename becomes the new attachment name; text is UTF-8 and must satisfy current preparation limits. Metadata company/role are retained as opaque company_ref/vacancy_ref. [Core input limits](json-contract.md#preparation-and-inspection) and current sender selection still apply.
