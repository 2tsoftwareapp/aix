# aix

A speckit-native AI desktop app built with Tauri (Rust + React).

aix turns the [spec-kit](https://github.com/github/spec-kit) methodology into a
desktop workflow: open a project, run the specify → clarify → plan → tasks →
implement pipeline with an AI agent, with human approval gates at every step.

## Features

- **Speckit pipeline** — constitution, specify, clarify, plan, tasks, implement,
  analyze, checklist as first-class steps with artifact-driven handoff
- **Multi-provider** — Anthropic, OpenAI, Gemini, OpenRouter and other
  OpenAI-compatible endpoints; custom model IDs supported
- **Approval gates** — ask / auto-edit / full-auto modes; destructive actions
  always require confirmation
- **Per-context workspaces** — drafts, transcripts and approvals are isolated
  per project + feature + step
- **Artifact editor** — read and edit spec artifacts in-app with draft safety

## Architecture

```
apps/desktop/
  src/            React + Astryx design system UI
  src-tauri/      Rust backend
    crates/
      speckit-core   pipeline steps, gates, runner state
      agent-loop     agent execution loop
      executor       tool execution with approval enforcement
      providers      LLM provider clients
      app-state      persisted app state
  speckit-templates/  vendored, SHA256-verified spec-kit templates
```

The backend logic is implemented in Rust crates; the app has no runtime
dependency on the original spec-kit CLI — only the template format is vendored
and hash-pinned.

## Development

Requires Node 22+, pnpm 9, Rust stable, and Tauri system dependencies.

```bash
pnpm install
pnpm dev            # run the desktop app in dev mode
pnpm build          # typecheck + production web build
pnpm test           # ui-conventions + regression checks
pnpm lint           # eslint
pnpm desktop:build  # produce platform bundle
(cd apps/desktop/src-tauri && cargo test --workspace)
```

## License

MIT — see [LICENSE](LICENSE). Vendored spec-kit templates retain their own
upstream license (`apps/desktop/speckit-templates/UPSTREAM-LICENSE`).
