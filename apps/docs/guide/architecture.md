# Architecture

## Primary runtime: the host coding agent

The plugin exposes canonical skills from `skills/`. A SessionStart hook injects
`using-aix`; the host selects domain/process skills and executes the eight-step engineering
spine with its own tools. There is no headless execution engine — the host agent is the runtime.

`skills/` is the canonical authoring source containing 160 skills in 9 categories.
`aix skills list` generates a local catalog; `aix skills validate` includes
invalid YAML/frontmatter and duplicate-name diagnostics instead of silently ignoring them.

## Products in the monorepo

| Surface | Path | Responsibility |
|---|---|---|
| Skills plugin | `skills/`, `hooks/`, plugin manifests | Methodology spine + domain skill library |
| Desktop app | `apps/desktop` | Speckit-native Tauri app (Rust backend, React/Astryx UI); independent of the TypeScript packages |
| Authoring toolchain | `packages/*` | Validation, compilation, and installation of skills to providers |

## Package map

| Package | Responsibility |
|---|---|
| `@x/core` | Shared types, phase machine and budget tracking |
| `@x/registry` | Canonical skill loader, Zod validation and catalog |
| `@x/policy` | Secret/PII redaction and shell-command denylist |
| `@x/providers` | Claude/Cursor/Codex/Gemini compile-time configuration adapters |
| `@x/compiler` | Hash-based idempotent file emission and conflict protection |
| `@x/cli` | Command surface: `install`, `skills`, `doctor` |

## Compile-time vs runtime

The TypeScript packages operate **before** the host session: they validate and compile
skill content into provider-specific formats (`.claude/`, `.codex/`, `.cursor/`, `GEMINI.md`).
At runtime the host agent reads those files directly — no aix code executes inside the
host session beyond the SessionStart hook that injects `using-aix`.

The desktop app implements its own runtime in Rust (`apps/desktop/src-tauri/crates`):
pipeline gates, provider streaming, tool-use loop, and audit logging. It shares no code
with the TypeScript toolchain but consumes the vendored speckit content in `content/speckit/`.

## Memory

Durable memory is plain Markdown in `.aix/runtime/memory/`, read and written by the host
agent with native file tools. See the `kb-workflow` skill for the canonical record format.
