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


const workspaceModulePath = new URL("../src/lib/workspaceSessions.ts", import.meta.url);
const {
  workspaceKey,
  upsertWorkspace,
  emptyWorkspace,
  appendStreamDelta,
  stepWorkspaceMatches,
} = loadTsModule(workspaceModulePath);

{
  const project = "/repo";
  const feature = "001-alpha";
  const specifyKey = workspaceKey(project, feature, "specify");
  const planKey = workspaceKey(project, feature, "plan");
  const otherFeatureKey = workspaceKey(project, "002-beta", "plan");
  assert.notEqual(specifyKey, planKey, "workspace keys must be separate per step");
  assert.notEqual(planKey, otherFeatureKey, "workspace keys must be separate per feature");

  const specifyContext = { projectRoot: project, featureSlug: feature, step: "specify" };
  const planContext = { projectRoot: project, featureSlug: feature, step: "plan" };
  let workspaces = {};
  workspaces = upsertWorkspace(workspaces, specifyKey, specifyContext, (ws) => ({ ...ws, draft: "spec draft" }));
  workspaces = upsertWorkspace(workspaces, planKey, planContext, (ws) => ({ ...ws, draft: "plan draft" }));
  assert.equal(workspaces[specifyKey].draft, "spec draft", "specify draft survives independently");
  assert.equal(workspaces[planKey].draft, "plan draft", "plan draft survives independently");

  workspaces = upsertWorkspace(workspaces, specifyKey, specifyContext, (ws) => appendStreamDelta(ws, "hello"));
  workspaces = upsertWorkspace(workspaces, specifyKey, specifyContext, (ws) => appendStreamDelta(ws, " world"));
  assert.equal(workspaces[specifyKey].messages.at(-1).text, "hello world", "stream deltas append to the originating assistant message");
  assert.equal(workspaces[planKey].messages.length, 0, "stream deltas do not leak into another step");

  assert.equal(stepWorkspaceMatches(workspaces[specifyKey], project, feature, "specify"), true, "originating step matches its workspace");
  assert.equal(stepWorkspaceMatches(workspaces[specifyKey], project, feature, "plan"), false, "other steps do not match the originating workspace");
  assert.deepEqual(emptyWorkspace(planContext).draft, "", "new workspaces start with an empty draft");

  const constitutionAlpha = { projectRoot: project, featureSlug: feature, step: "constitution" };
  const constitutionBeta = { projectRoot: project, featureSlug: "002-beta", step: "constitution" };
  assert.equal(
    workspaceKey(project, feature, "constitution"),
    workspaceKey(project, "002-beta", "constitution"),
    "constitution is a project-level workspace across features",
  );
  assert.equal(emptyWorkspace(constitutionAlpha).featureSlug, null, "constitution workspace state is normalized to the project");
  workspaces = upsertWorkspace(workspaces, workspaceKey(project, feature, "constitution"), constitutionBeta, (ws) => ws);
  assert.equal(
    workspaces[workspaceKey(project, feature, "constitution")].featureSlug,
    null,
    "upserting constitution from another feature keeps project-level ownership",
  );
}

const chatPanelSource = readFileSync(new URL("../src/components/ChatPanel.tsx", import.meta.url), "utf8");
const artifactPanelSource = readFileSync(new URL("../src/components/ArtifactPanel.tsx", import.meta.url), "utf8");

{
  assert.match(chatPanelSource, /const effectiveModel = typedModel \|\| model/, "run_step uses the typed model when the user does not open the menu");
  assert.match(chatPanelSource, /model: effectiveModel/, "the effective typed model is sent to the backend");
  assert.doesNotMatch(
    chatPanelSource,
    /setMessages\(\[\]\);\s*\}, \[step, projectRoot, featureSlug\]\);/,
    "context switches must not reset the transcript because workspaces are keyed per step",
  );
  assert.match(chatPanelSource, /run_id: runId/, "run_step sends a client-generated run id");
  assert.match(chatPanelSource, /updateRunWorkspace/, "stream events update the originating workspace rather than the visible step");
  assert.match(chatPanelSource, /registerRun\(runId, key\)/, "runs are registered against their workspace before invocation");
  assert.match(chatPanelSource, /rekeyRunWorkspace\(p\.run_id, p\.feature_slug\)/, "specify workspaces follow the feature created by the backend");
  assert.match(chatPanelSource, /workspaceContextForRun\(p\.run_id\)/, "done handling uses the pinned run context rather than the visible step");
  assert.doesNotMatch(chatPanelSource, /kind === "done"[\s\S]{0,500}failedPrompt: null/, "a trailing done event must not erase a failed prompt restored for retry");
  assert.match(chatPanelSource, /cancel_run", \{ runId: activeRunId \}/, "stop cancellation targets the run displayed in the current workspace");
}
{
  assert.match(artifactPanelSource, /target\.key !== identityKey/, "saves are bound to the currently loaded artifact identity");
  assert.match(artifactPanelSource, /draftsRef\.current\.get\(identityKey\)/, "artifact drafts are restored by project-feature-file identity");
  assert.match(artifactPanelSource, /loadGenerationRef\.current !== generation/, "stale artifact loads cannot overwrite the active file buffer");
}


const appSource = readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8");
const commandsSource = readFileSync(new URL("../src-tauri/src/commands.rs", import.meta.url), "utf8");
{
  assert.match(appSource, /if \(selectNewest\) return state\.features\[state\.features\.length - 1\]/, "specify completion selects the newest created feature");
  assert.match(appSource, /setActiveStep\("specify"\)/, "specify completion returns to the workspace whose spec is opened");
  assert.match(appSource, /completedFeatureSlug != null/, "failed specify runs do not trigger created-feature navigation");
  assert.match(commandsSource, /RunContext \{ feature_slug: Option<String> \}/, "backend streams the feature target pinned to each run");
  assert.match(commandsSource, /emit\(StepStreamPayload::RunContext/, "backend emits run context before streaming output");
  assert.match(commandsSource, /specify_context_pending/, "specify migration waits until the new feature artifact exists so failed runs keep their prompt in the originating workspace");
  assert.match(commandsSource, /Done \{ feature_slug: Option<String> \}/, "done events report whether a feature artifact was actually committed");
}

const sidebarSource = readFileSync(new URL("../src/components/Sidebar.tsx", import.meta.url), "utf8");
{
  assert.match(sidebarSource, /stepWorkspaceMatches/, "sidebar finds run state at the originating workspace step");
  assert.match(sidebarSource, /approval pending/, "sidebar exposes pending approval indicators");
  assert.match(sidebarSource, /blockedTarget/, "blocked pipeline steps expose prerequisite navigation");
  assert.match(sidebarSource, /case "clarify":\s*return "none"/, "spec.md existing does not mark clarify as completed");
  assert.match(sidebarSource, /\/3 docs/, "feature rows distinguish artifact/document presence from run state");
}

console.log("regression helper tests passed");
