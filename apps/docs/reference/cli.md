# CLI Reference

Packages are private. Build the checkout, then use `node packages/cli/dist/index.js` wherever
examples below say `aix`. Run `aix <command> --help` for authoritative flags.

## Provider installation

```bash
aix install --provider claude
aix install --provider cursor --dry-run
aix install --all
```

Default provider is Claude. `--all` selects all four providers; it is not a filter for
`--provider`. Existing unmarked user files are conflicts unless `--force` is explicitly used.

## Skill catalog

```bash
aix skills validate [--content-root <content-directory>]
aix skills list
aix skills show <name>
```

`skills/` is the canonical authoring source (160 skills in 9 categories). `list` regenerates
the local catalog from canonical content; `validate` reports invalid YAML/frontmatter and
duplicate identifiers.

## Doctor

```bash
aix doctor
```

Checks the local workspace setup: build outputs, catalog freshness, and provider install state.

## Removed commands

The headless execution surface (`run`, `verify`, `memory`, `kb`, `context`, `eval`) was removed
when the TypeScript execution stack was retired. Pipeline execution now lives in the desktop
app (`apps/desktop`, Rust/Tauri); durable memory is plain Markdown in `.aix/runtime/memory/`
managed directly by the host agent.
