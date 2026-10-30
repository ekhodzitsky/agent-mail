# Offline package workload

The measured promise is local preparation, review and no-network validation of a frozen package. This is a correctness-qualified latency sample, not a comparison with another mail client. Run date: 2026-10-30.

## Reproduce

```sh
python3 -B scripts/benchmark-offline.py --binary /absolute/path/to/agent-mailer \
  --packages 20 --runs 3
```

The standard-library Python driver creates a new mode-700 temporary home for each run, writes example.org sender/recipient identities, a 37-byte final plain-text letter and a 4,335-byte synthetic PDF-prefix fixture. Each run starts 20 prepare, 20 inspect and 20 configured dry-run subprocesses sequentially. No actual send, credentials, socket test or real mailbox is used. SMTP is syntactically configured for verified TLS on numeric loopback only; no server is needed. JSON responses and exit success are required. Each dry-run must report `network_used:false`; archive SHA-256 before/after all dry-runs must match. Temporary data is removed.

Command times include process startup, SQLite open/validation, JSON serialization/parsing and pipe collection; preparation includes the real SQLite commit. Group elapsed time is divided by command count. This is not an in-process microbenchmark or a per-request tail-latency estimate.

## Recorded results

Agent Mailer 0.7.1 normalized production binary; SHA-256 and full driver output are recorded in [raw results](benchmarks-offline.json). The release verification records its exact source revision. Rust release build on Linux x86_64, AMD Ryzen AI 9 HX 370, 24 logical CPUs; Python 3.14.4. Temporary state used tmpfs, normal warm OS caches, default SQLite DELETE journal/EXTRA synchronous behavior and no pinned CPU/power settings. Compilation is excluded. No CPU governor, filesystem flush, memory or cold-start experiment was performed.

| Operation | Run means, ms/command | Median of means, ms ↓ | Range, ms |
|---|---|---:|---:|
| Prepare | 3.4955, 3.4276, 3.3196 | 3.4276 | 3.3196–3.4955 |
| Inspect | 3.4943, 3.2223, 3.0798 | 3.2223 | 3.0798–3.4943 |
| Dry-run | 3.2977, 3.2308, 3.4651 | 3.2977 | 3.2308–3.4651 |

All 60 preparations/reviews/dry-runs succeeded and all three archives remained byte-identical across dry-run. Values are rounded from driver JSON. No alternative was run, so no speedup or safety superiority is claimed. The [local validation suite](validation.md) separately exercises refusal, concurrency, crashes and immutable bytes; latency alone does not establish these guarantees.

## Limits

This tiny workload is a lower-cost case: no HTML, large PDF, many attachments, reports or large archive. tmpfs does not represent persistent-disk fsync latency or power-loss behavior. Group means hide individual outliers. Repeated calls warm caches; host load and frequency scaling are uncontrolled. Do not generalize to large-archive listing, real SMTP/TLS latency, delivery time or sustained throughput. Bounded listing and realistic persistent-storage workloads remain [maintenance tasks](maintenance.md).

The driver emits machine-readable raw samples with its workload and version. Preserve exact source revision, binary hash and JSON alongside release evidence when using a new release; results from a development source must not be attributed to an unchanged older release asset.

## Archive listing (0.6.0)

Run date: 2026-10-30. Same Linux x86_64 / Ryzen AI 9 HX 370 host as above; fresh synthetic 300-package archive with a 256 KiB PDF per package (75 MiB of source PDFs), plain letter, no attempts or reports, tmpfs state and warm/uncontrolled OS caches. Production normalized 0.5.2 and 0.6.0 binaries; exact hashes and all repeated latency/RSS samples are in [raw listing results](benchmarks-list.json).

```sh
python3 -B scripts/benchmark-list.py --old /path/to/agent-mailer-0.5.2 \
  --new /path/to/agent-mailer-0.6.0 --packages 300 --runs 3
```

Requires system Python and GNU `/usr/bin/time`, exclusively for this optional benchmark. The driver prepares through the actual prior binary, then runs old full, new full, new verified page and new ID summary in fixed order three times. It asserts identical full results, exact first-page equivalence, expected unverified summary shape and unchanged archive bytes. No network/credentials/owner files. Wall latency includes process/GNU time startup and JSON parsing; GNU time maximum resident set covers the CLI process, not the Python driver. Table latency is median of run times; RSS is median of per-run peak KiB, lower is smaller.

| Binary / operation | Returned packages | Work | Median ms ↓ | Run range ms | Peak RSS KiB ↓ |
|---|---:|---|---:|---:|---:|
| 0.5.2 full | 300 | Full status | 164.07 | 149.52–172.77 | 10428 |
| 0.6.0 full | 300 | Full status | 149.78 | 144.56–151.40 | 10368 |
| 0.6.0 page | 25 | Full selected status | 20.33 | 19.01–20.46 | 7968 |
| 0.6.0 summary | 25 | IDs only; not verified | 4.05 | 3.99–4.18 | 7232 |

Only full-to-full rows do equivalent work. Pagination deliberately selects 25 packages; summary does not verify materials or report history. Faster reduced work is **not** a safety-equivalent full-archive speedup. The results do not establish improved full-archive latency; retain unfavorable full-to-full samples rather than inferring a general optimization. Pages bound returned package count, not arbitrary per-package history or PDF size.

This workload has no HTML, many attachments, repeated refusals or DSNs. It is not a memory ceiling, persistent-disk benchmark, concurrent-writer study or large-archive guarantee. All status rows can change between page invocations; keyset pagination is not a frozen multi-page snapshot. See [CLI rules](cli.md#archive-pages-060) and [maintenance](maintenance.md).

## Persistent-filesystem backup (0.7.0)

Run date: 2026-10-30. Native normalized 0.7.0, same Linux x86_64/Ryzen host as above, **ext4 on NVMe**, 500 synthetic packages with one 256 KiB PDF and plain letter each. Source database: 311963648 bytes. Three sequential backup/verify runs to new directories on that same filesystem; each copy is removed after verification. System caches are warm/uncontrolled, CPU/load unpinned. Exact artifact hash and all samples: [raw backup results](benchmarks-backup.json).

```sh
python3 -B scripts/benchmark-backup.py --binary /path/to/agent-mailer-0.7.0 \
  --work-parent /private/synthetic-benchmark-parent --packages 500 --runs 3
```

Requires Python 3.11+, GNU time and findmnt solely for the optional driver. Select an existing parent on the filesystem being measured; the driver checks free space, creates its own temporary synthetic child, keeps one snapshot at a time and removes only its generated data. It never fills a filesystem deliberately. No owner letters, credentials, SMTP or IMAP are used. A source hash before/after must match, and every snapshot operation must report verified integrity, no network, no retry permission and freshness:not_established.

| 0.7.0 operation | Median ms ↓ | Run range ms | Median per-run peak RSS KiB ↓ |
|---|---:|---:|---:|
| backup | 864.30 | 863.72–874.32 | 12584 |
| verify | 416.06 | 413.98–420.05 | 10648 |

Latency includes CLI/GNU time startup and JSON parsing. Backup includes source validation, SQLite copy, destination validation/hashing, file/directory fsync and manifest publication. Verify includes manifest/hashes, database checks and full stored-material validation. RSS is GNU time's CLI-process peak, excluding the driver. Preparation is excluded. These repeated measurements describe one modest plain archive; no cache, HTML, attempts or reports are in this workload.

This is persistent-filesystem behavior, not a power-cut or storage-firmware durability test. It does not establish cold-cache latency, concurrent-writer performance, an RSS ceiling or all larger archives. The separate deterministic fault tests cover process death, SQLite page exhaustion and per-process file-size quota; quota/EFBIG is not physical ENOSPC. Read locks can still delay writers, so quiesce active mail workers before operational backup. No comparison against another backup product was performed.
