# Publication checklist

Publish only reviewed source, synthetic fixtures and verified release assets. Operational configuration, credentials, correspondence, personal documents, mail databases and private development history stay outside this repository. Publishing code never authorizes sending mail or accessing a mailbox.

1. Review the complete tracked tree, commit messages and tags. Scan secrets and configured private-identity markers without adding the marker inputs themselves to Git. Keep generated agent state, logs, caches, database files and executable backups ignored. Public repository ownership and intentional Git author attribution are distinct from personal deployment data.
2. Build source archives from the reviewed tag and compare member names/bytes against its tree. Include no .git directory, private refs, source repositories, ignored data or operational evidence. Never push a historical private clone, mirror or bundle to this repository.
3. Run local stable/MSRV/default/test-SMTP/adapter gates and relevant failure/recovery checks. Keep hosted Actions disabled unless the owner explicitly changes that instruction. Production binaries exclude test-smtp and internal fault fixtures.
4. Build normalized native assets, inspect version/features/path markers, verify hashes and installation instructions, and publish exact-source verification with scoped limitations. Checksums are not independent signed provenance.
5. Maintain README/license/support/contribution/security documentation and enable private vulnerability reporting. Never test reporting by posting sensitive details in a public issue.
6. Check the resulting public repository, tag source and downloadable assets. Record actual checks and limitations. Keep real-mail authorization, runtime state and backup freshness separate from source publication.

Once material is public, deleting refs or assets cannot recall every clone, cache or download. Prevent accidental publication before pushing rather than relying on cleanup afterward.
