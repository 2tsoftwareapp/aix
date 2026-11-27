---
layout: home

hero:
  name: aix
  text: Author Once,<br>Compile to Every Provider
  tagline: Engineering methodology and canonical skills for host coding agents, plus a speckit-native desktop app.
  actions:
    - theme: brand
      text: Get Started
      link: /guide/getting-started
    - theme: alt
      text: CLI Reference
      link: /reference/cli

features:
  - icon: 🎯
    title: Canonical Skills
    details: Write SKILL.md once. Compile to Claude Code, Cursor, Codex, and Gemini CLI automatically — no manual sync.

  - icon: 🖥️
    title: Speckit-Native Desktop
    details: Tauri desktop app implementing the speckit pipeline in Rust — gates, provider streaming, approval-gated tool use, artifact editing.

  - icon: 🧠
    title: Markdown Memory
    details: Durable memory as plain Markdown in .aix/runtime/memory/ — no service to install, searchable with standard file tools.

  - icon: 🔒
    title: Policy & Redaction
    details: Session and memory strings are sanitized before persistence; generated source with detected secrets is rejected rather than silently rewritten.

  - icon: 📦
    title: Canonical Skill Library
    details: Domain skills, process workflows, and reference guides authored under skills/ with metadata, instructions, and on-demand resources.

  - icon: ⚡
    title: pnpm + Turborepo
    details: 6 TypeScript authoring packages plus desktop and docs workspaces, with dependency-aware builds and test commands.
---
