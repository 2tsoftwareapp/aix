import type { StepId } from "../types";

export type Msg =
  | { role: "user"; text: string }
  | { role: "assistant"; text: string }
  | { role: "system"; variant: "info" | "success" | "error" | "warning" | "tool"; text: string };

export interface ApprovalReq {
  approval_id: string;
  tool: string;
  detail: string;
  preview: string | null;
  step: StepId;
  projectRoot: string;
  featureSlug: string | null;
  runId?: string;
}

export interface WorkspaceContext {
  projectRoot: string;
  featureSlug: string | null;
  step: StepId;
}

export interface WorkspaceState extends WorkspaceContext {
  key: string;
  draft: string;
  messages: Msg[];
  approvals: ApprovalReq[];
  running: boolean;
  activeRunId: string | null;
  failedPrompt: string | null;
  lastPrompt: string | null;
}

export type WorkspaceMap = Record<string, WorkspaceState>;

export function workspaceKey(projectRoot: string, featureSlug: string | null, step: StepId): string {
  const scopedFeature = step === "constitution" ? null : featureSlug;
  return JSON.stringify([projectRoot, scopedFeature, step]);
}

export function normalizeWorkspaceContext(context: WorkspaceContext): WorkspaceContext {
  return {
    ...context,
    featureSlug: context.step === "constitution" ? null : context.featureSlug,
  };
}

export function emptyWorkspace(context: WorkspaceContext): WorkspaceState {
  const normalized = normalizeWorkspaceContext(context);
  return {
    ...normalized,
    key: workspaceKey(normalized.projectRoot, normalized.featureSlug, normalized.step),
    draft: "",
    messages: [],
    approvals: [],
    running: false,
    activeRunId: null,
    failedPrompt: null,
    lastPrompt: null,
  };
}

export function upsertWorkspace(
  workspaces: WorkspaceMap,
  key: string,
  context: WorkspaceContext,
  updater: (workspace: WorkspaceState) => WorkspaceState,
): WorkspaceMap {
  const normalized = normalizeWorkspaceContext(context);
  const current = workspaces[key] ?? { ...emptyWorkspace(normalized), key };
  return { ...workspaces, [key]: updater({ ...current, ...normalized, key }) };
}

export function appendStreamDelta(workspace: WorkspaceState, text: string): WorkspaceState {
  const messages = [...workspace.messages];
  const last = messages[messages.length - 1];
  if (last?.role === "assistant") {
    messages[messages.length - 1] = { ...last, text: last.text + text };
  } else {
    messages.push({ role: "assistant", text });
  }
  return { ...workspace, messages };
}

export function appendMessage(workspace: WorkspaceState, message: Msg): WorkspaceState {
  return { ...workspace, messages: [...workspace.messages, message] };
}

export function stepWorkspaceMatches(
  workspace: WorkspaceState,
  projectRoot: string,
  featureSlug: string | null,
  step: StepId,
): boolean {
  return workspace.projectRoot === projectRoot
    && workspace.step === step
    && (step === "constitution" || workspace.featureSlug === featureSlug);
}
