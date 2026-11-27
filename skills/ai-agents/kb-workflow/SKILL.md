---
name: kb-workflow
description: >-
  Memory lifecycle for agents: search before work, write structured solutions after
  solving problems. Memory is plain Markdown files in .aix/runtime/memory/ — always
  available, no install or service required. Use this skill when saving a solution,
  documenting a fix, searching for existing knowledge, or managing agent memory.

  Triggers: "kb", "knowledge base", "solution", "push", "search knowledge", "save this",
  "document this", "write solution", "skill kb", "memory", "remember this"
x-kind: domain
x-version: 0.3.0
x-tags: [memory, knowledge-base, markdown, solution, search]
x-roles: [coder, architect, planner]
x-compatible: [claude, cursor, codex, gemini]
x-category: ai-agents
---

# KB Workflow

Memory lifecycle — search before work, write structured solutions after.

## Storage

Memory lives in **`.aix/runtime/memory/`** as plain `.md` files — one file per record,
YAML frontmatter + body. No service, no install: the host agent reads and writes these
files directly with standard file tools.

```
.aix/runtime/memory/
  <id>.md   # one file per record; frontmatter + body
```

**Use memory for:**
- Session notes, decisions, gotchas — things you want the next session to find
- Bug fixes, patterns, quick references
- Any knowledge that fits in a few hundred words

**Searching memory** — use grep/glob over the directory:
```bash
grep -ril "nestjs initialization" .aix/runtime/memory/
ls .aix/runtime/memory/
```

## Boundary

- **This skill** covers the memory/solution lifecycle: search, write, update, delete.
- Defers to **skill-authoring** for creating SKILL.md files.
- Defers to **deploy-workflow** for VPS/server deployment operations.

## When to use

- Before starting new work (search first — avoid reinventing)
- After solving a bug, infrastructure issue, or discovering a pattern
- After completing a ticket or feature
- When someone asks "how did we solve X before?"
- When documenting a decision or trade-off

## Workflow

### Search first

```bash
grep -ril "nestjs initialization" .aix/runtime/memory/
```

### Write a solution (canonical format)

```markdown
# NestJS Initialization Race Condition Fix

## Problem
onModuleInit hooks fired concurrently when multiple services depended on a shared resource.

## Solution
Centralize initialization in the lowest-level service. All dependents inject and await it.

```typescript
@Injectable()
export class DatabaseService implements OnModuleInit {
  async onModuleInit() {
    await this.connect(); // runs once; dependents inject this service
  }
}
```

## Key Insight
NestJS does not guarantee initialization order across peer services at the same depth.
Centralize in the deepest dependency.

## Tags
nestjs, race-condition, initialization, onModuleInit

## Technologies
NestJS, TypeScript
```

### Save to memory

Write the record directly as a Markdown file with frontmatter:

```bash
cat << 'EOF' > .aix/runtime/memory/nestjs-race-fix.md
---
title: NestJS Initialization Fix
tags: [nestjs, race-condition, initialization]
createdAt: 2026-11-05
---
Centralize onModuleInit in lowest service.
EOF
```

Use a kebab-case id filename derived from the title. If a record for the same problem
already exists, update that file in place instead of creating a near-duplicate.

## Solution format

Every solution follows: **H1 Title → Problem → Solution (with code) → Key Insight → Tags → Technologies**.

| Scenario | Format |
|---|---|
| Bug fix | Problem (error message) → Root cause → Fix (code) → Prevention |
| Infrastructure | Context → Steps → Final working config → Gotchas |
| Pattern | When to use → Implementation → Trade-offs → Alternatives |
| Decision record | Context → Options → Decision → Rationale |

## Operating principles

1. **Search before writing** — check existing records first.
2. **One solution per problem** — no combined fixes, no near-duplicates.
3. **Tag richly** — specific tags make grep-based discovery reliable.
4. **Include code** — solutions without code examples are incomplete for a developer KB.
5. **Update, don't duplicate** — if a solution exists, update it rather than creating a near-copy.
6. **Vietnamese acceptable** — solution content may be in Vietnamese; tags should be English.

## Anti-patterns

- **No tags** — solution is orphaned, never discovered
- **Wall of text** — missing structure makes scanning impossible
- **Skipped search** — creates duplicate fragments
- **Overly broad** — "How to use Docker" is not a solution; be specific

## Resources

| Topic | File |
|---|---|
| Solution format | [references/solution-format.md](references/solution-format.md) |
| Search strategies | [references/search-strategies.md](references/search-strategies.md) |
| Anti-patterns | [references/anti-patterns.md](references/anti-patterns.md) |

## Checklist before done

- [ ] Searched `.aix/runtime/memory/` before writing (no duplicates created)
- [ ] Solution follows canonical format (Problem → Solution → Key Insight → Tags)
- [ ] Tags are rich and specific
- [ ] Code examples included
- [ ] Record is discoverable via `grep -ril "<topic>" .aix/runtime/memory/`
