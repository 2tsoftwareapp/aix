# Tổng quan dự án aix

## Mục tiêu và mô hình thực thi

**Host agent là runtime.** aix cung cấp phương pháp kỹ thuật tám bước và thư viện skill;
không có engine headless. Thư viện canonical hiện có **160 SKILL.md** được tổ chức theo
9 category trực tiếp trong [skills/](skills/).

Repo gồm ba bề mặt sản phẩm:

| Bề mặt | Đường dẫn | Vai trò |
|---|---|---|
| Skills plugin | [skills/](skills/), [hooks/](hooks/), [.claude-plugin/](.claude-plugin/), [.codex-plugin/](.codex-plugin/) | Spine phương pháp + thư viện skill cho Claude Code / Codex |
| Desktop app | [apps/desktop/](apps/desktop/) | Ứng dụng Tauri speckit-native (backend Rust + UI React/Astryx) — pipeline execution, approvals, artifact editing |
| Authoring toolchain | [packages/](packages/) | Validate, compile, install skills sang các provider |

## Cấu trúc

| Khu vực | Vai trò |
|---|---|
| [.claude-plugin/](.claude-plugin/) và [.codex-plugin/](.codex-plugin/) | Manifest plugin |
| [hooks/](hooks/) | Inject skill nhập môn qua SessionStart |
| [skills/](skills/) | Nguồn canonical của 160 skills phân theo 9 category |
| [content/](content/) | Rule, workflow, và speckit templates vendored cho desktop |
| [packages/](packages/) | 6 package TypeScript authoring quản lý bằng pnpm/Turborepo |
| [apps/desktop/](apps/desktop/) | Sản phẩm chính — Tauri desktop app |
| [apps/docs/](apps/docs/) | Tài liệu VitePress |
| [docs/history/](docs/history/) | Archive migration/assessment đã hoàn thành |

Các package authoring: `@x/core` (type/phase/budget), `@x/registry` (schema/catalog),
`@x/policy` (redaction/denylist), `@x/providers` (compile-time adapters), `@x/compiler`
(emitter) và `@x/cli` (install/skills/doctor). Chi tiết tại [Architecture](apps/docs/guide/architecture.md).

## Desktop app

`apps/desktop` implement speckit pipeline native bằng Rust (`src-tauri/crates`): gate engine,
provider streaming (Anthropic, OpenAI, Qwen, OpenRouter), tool-use loop có approval gating,
cancellation, audit logging, artifact management. Frontend React + Astryx. App không phụ
thuộc package TypeScript nào; speckit templates lấy từ `content/speckit/` và được verify
SHA256 khi load.

```bash
pnpm --filter @aix/desktop build
cd apps/desktop/src-tauri && cargo test --workspace
```

## Engineering spine của host

1. Align & Shape — `discussing-pro`.
2. Plan — `planning-pro`.
3. Isolate — `git-worktree-pro`.
4. Test-first — `ttd-pro`.
5. Execute — `executing-pro`.
6. Review — `code-review-pro`.
7. Verify — `verify-pro`.
8. Remember — `remember-pro`.

Đây là workflow hướng dẫn host agent. Artifact của một phiên nằm trong `.aix/sessions/<session-id>/`.

## Memory và an toàn dữ liệu

Memory là plain Markdown trong `.aix/runtime/memory/` — host agent đọc/ghi trực tiếp bằng
file tool, không cần service nào. Xem skill `kb-workflow` cho format canonical.

Trong authoring toolchain, nội dung nhạy cảm được redact trước khi ghi; source có secret
nhận diện được bị từ chối thay vì bị sửa âm thầm. Desktop app lưu provider credential trong
XDG config JSON (0600 trên unix), env var làm fallback, và gate mọi tool mutation qua
approval của người dùng. Tham chiếu [CLI Reference](apps/docs/reference/cli.md).
