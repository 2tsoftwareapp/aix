# aix

> AI engineering methodology + 160 canonical skills across 9 categories, delivered as a Claude Code and Codex plugin — plus a speckit-native desktop app.

**The host agent is the runtime**. aix is the playbook and the skill library — not a separate engine you invoke.

## Products in this repo

| Surface | Path | What it is |
|---------|------|------------|
| Skills plugin | `skills/`, `hooks/`, `.claude-plugin/`, `.codex-plugin/` | Methodology spine + domain skill library for Claude Code / Codex |
| Desktop app | `apps/desktop` | Speckit-native Tauri desktop app (Rust backend + React/Astryx UI) — pipeline execution, approvals, artifact editing |
| Authoring toolchain | `packages/*` | TypeScript packages for validating, compiling, and installing skills to providers |

## How the plugin works

```bash
git clone https://github.com/truongnat/aix
```

Claude Code:

```text
/plugin install /path/to/aix
```

Codex:

```bash
codex plugin marketplace add truongnat/aix
codex plugin add aix@aix
```

On start, a `SessionStart` hook injects [`using-aix`](./skills/methodology/using-aix/SKILL.md) into context. It explains the 8-step engineering spine and how to reach the canonical skills via the host's `Skill` tool.

No engine, no runtime — just skills the agent pulls in when needed.

## The engineering spine

| # | Phase | Skill |
|---|-------|-------|
| 1 | Align & Shape | `discussing-pro` |
| 2 | Plan | `planning-pro` |
| 3 | Isolate | `git-worktree-pro` |
| 4 | Test-first | `ttd-pro` |
| 5 | Execute | `executing-pro` |
| 6 | Review | `code-review-pro` |
| 7 | Verify | `verify-pro` |
| 8 | Remember | `remember-pro` |

## Desktop app

`apps/desktop` is a Tauri 2 application implementing the speckit pipeline natively in Rust:
project scanning, gate engine, provider streaming (Anthropic, OpenAI, Qwen, OpenRouter),
tool-use loop with approval gating, cancellation, audit logging, and artifact management.
Its frontend is React + Astryx. It does not depend on the TypeScript packages.

```bash
pnpm --filter @aix/desktop build     # web bundle typecheck + build
cd apps/desktop/src-tauri && cargo test --workspace
```

Speckit templates are vendored under `content/speckit/` and SHA256-verified at load.

## Authoring toolchain

6 packages for authoring, validating, compiling, and installing skills:

| Package | Role |
|---------|------|
| `@x/registry` | SKILL.md schema (Zod), loader, catalog |
| `@x/policy` | Secret/PII redaction, shell denylist |
| `@x/providers` | Compile-time provider adapters (Claude, Cursor, Codex, Gemini) |
| `@x/compiler` | Idempotent file emitter |
| `@x/cli` | Command surface (`install`, `skills`, `doctor`) |
| `@x/core` | Types, phase machine, guards |

## Quick start

```bash
pnpm install --frozen-lockfile --ignore-scripts
pnpm build
pnpm test
```

## CLI

```bash
aix install --all
aix install --provider cursor
aix skills validate [--content-root <content-directory>]
aix skills list
aix skills show <name>
aix doctor
```

The workspace packages are private; build locally and use `node packages/cli/dist/index.js`
in place of `aix` unless the executable is installed locally.

`skills/` is the canonical authoring source containing 160 skills organized in 9 categories.
`aix skills list` regenerates the local catalog from canonical content;
validation includes invalid YAML/frontmatter and duplicate identifiers.

## Repository

```
.agents/plugins/  Codex marketplace entry for this repo
.claude-plugin/   Claude Code plugin manifest
.codex-plugin/    Codex plugin manifest
hooks/            SessionStart → injects using-aix
skills/           Canonical domain, process, and reference skills in 9 categories
content/
  rules/          Spine guardrail rules
  workflows/      Engineering spine flow
  speckit/        Vendored speckit templates/extensions for the desktop app
packages/         6 @x/* TypeScript authoring packages
apps/
  desktop/        Speckit-native Tauri desktop app
  docs/           Documentation site
docs/history/     Completed migration/assessment archives
```

## Security

- Sensitive session prose and memory content are redacted before persistence in the
  authoring toolchain. Secret-bearing identifiers/paths are rejected instead of silently rewritten.
- The desktop app stores provider credentials in an XDG config JSON file (0600 on unix)
  with env-var fallback, and gates mutating tool calls behind explicit user approval.
- A shell denylist is not a substitute for an OS security sandbox.

## License

MIT
