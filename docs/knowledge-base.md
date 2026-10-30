# Development knowledge base

Managed development instructions are installed from a separately maintained handbook, version 0.10.0. Development-source access is configured separately and is not required to use or build the distributed project. Building or using the CLI does not require access to that source.

The prompt-based installation is used instead of a runtime dependency or copied repository. Mandatory managed Rust coding principles are installed in `agents/coding-principles.md` and linked from AGENTS.md. Local-only rules are installed in the handbook and .gitignore. The stamp records the installed instruction version, not optional tool connectivity.

Maintainers with access to the configured source should read its README, POLICY, SCHEMA, TARGET-LAYOUT and orchestrator before updating. Run its read-only `scripts/principles.py --target /absolute/project --rust` planner, review changes and apply them only if original bytes match. Preserve local additions outside the managed block. A current stamp alone is not evidence that the body is unchanged. Optional tool setup is sequential and requires live connection evidence; do not infer readiness from configuration files.

Machine-specific MCP configuration, indexes, sessions and backlog stay local. They are not shipped with the CLI. The core product has no dependency on Python, Node.js, Bun, MCP or a task-tracker service.

## Root documentation reference

The root README follows the installed handbook’s Root README standard v1 and skeleton: factual opening, useful synthetic first run, accurate badges, primary-source comparison, reproducible scoped evidence and visible adoption limits. The reference is a development guideline, not a runtime dependency. Keep detailed commands in [CLI reference](cli.md), routes in the [documentation index](README.md) and source/version qualifications beside the relevant claim. No CI/coverage/download badge may imply an unavailable measurement.
