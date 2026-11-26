import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import ts from "typescript";

function loadTsModule(path) {
  const source = readFileSync(path, "utf8");
  const output = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  const module = { exports: {} };
  new Function("exports", "module", output)(module.exports, module);
  return module.exports;
}

const { stripMarkdownComments } = loadTsModule(new URL("../src/lib/markdown.ts", import.meta.url));

{
  const input = "Use `<!-- marker -->` here.\n";
  assert.equal(stripMarkdownComments(input), input, "inline code must keep comment-like text verbatim");
}
{
  const input = "before\n\n  ```markdown\n  <!-- sample comment -->\n  ```\n\nafter\n";
  assert.equal(stripMarkdownComments(input), input, "indented fenced code must keep sample comments verbatim");
}
{
  const input = "<!-- template guidance -->\n# Visible\n";
  assert.equal(stripMarkdownComments(input), "# Visible\n", "template comments outside code are removed");
}
{
  const input = "before\n~~~\n<!-- sample -->\n~~~\nafter\n";
  assert.equal(stripMarkdownComments(input), input, "tilde fences keep sample comments verbatim");
}

const { searchModelItems } = loadTsModule(new URL("../src/lib/models.ts", import.meta.url));

{
  const items = searchModelItems(["claude-sonnet-4-5", "claude-opus-4-5"], "sonnet");
  assert.deepEqual(items.map((item) => item.id), ["claude-sonnet-4-5", "sonnet"], "model search ranks catalog matches before a custom typed id");
}
{
  const items = searchModelItems(["claude-sonnet-4-5"], "custom-model-2026");
  assert.equal(items[0]?.id, "custom-model-2026", "unknown typed model remains selectable");
  assert.equal(items[0]?.auxiliaryData?.custom, true, "typed model is marked as custom");
}
{
  const items = searchModelItems(["claude-sonnet-4-5"], "claude-sonnet-4-5");
  assert.equal(items.filter((item) => item.id === "claude-sonnet-4-5").length, 1, "exact catalog matches are not duplicated");
}
{
  const items = searchModelItems(["a", "b"], "");
  assert.deepEqual(items.map((item) => item.id), ["a", "b"], "empty query lists available models");
}

const chatPanelSource = readFileSync(new URL("../src/components/ChatPanel.tsx", import.meta.url), "utf8");
const artifactPanelSource = readFileSync(new URL("../src/components/ArtifactPanel.tsx", import.meta.url), "utf8");

{
  assert.match(chatPanelSource, /const effectiveModel = typedModel \|\| model/, "run_step uses the typed model when the user does not open the menu");
  assert.match(chatPanelSource, /model: effectiveModel/, "the effective typed model is sent to the backend");
  assert.match(
    chatPanelSource,
    /useEffect\(\(\) => \{\s*setMessages\(\[\]\);\s*\}, \[step, projectRoot, featureSlug\]\);/,
    "context switches reset messages without clearing pending approval cards",
  );
}
{
  assert.match(artifactPanelSource, /target\.key !== identityKey/, "saves are bound to the currently loaded artifact identity");
  assert.match(artifactPanelSource, /draftsRef\.current\.get\(identityKey\)/, "artifact drafts are restored by project-feature-file identity");
  assert.match(artifactPanelSource, /loadGenerationRef\.current !== generation/, "stale artifact loads cannot overwrite the active file buffer");
}

console.log("regression helper tests passed");
