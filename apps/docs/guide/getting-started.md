# Getting Started

## Requirements

- A current Node 22 release (22.13 or newer) or Node 24.
- pnpm 9.15.0, as pinned by the workspace.
- For the desktop app: a Rust toolchain and Tauri 2 system dependencies.

## Build the private workspace

```bash
git clone https://github.com/truongnat/aix
cd aix
pnpm install --frozen-lockfile --ignore-scripts
pnpm build
pnpm lint
pnpm test
```

These packages are not published npm installation targets. Use the local built CLI:

```bash
node packages/cli/dist/index.js skills validate
node packages/cli/dist/index.js install --provider cursor --dry-run
pnpm run doctor
```

`skills/` is the canonical library. The primary Claude/Codex plugin workflow uses
the host agent as runtime; see the repository README for plugin installation commands.

## Desktop app

```bash
pnpm --filter @aix/desktop build
cd apps/desktop/src-tauri && cargo test --workspace
```

## Memory

Memory is plain Markdown in `.aix/runtime/memory/` — no service required. Search it with
standard file tools before starting work; write structured solution records after solving
problems. See the `kb-workflow` skill for the canonical format.
