import { useSyncExternalStore } from "react";
import {
  appendMessage,
  normalizeWorkspaceContext,
  workspaceKey,
  upsertWorkspace,
  type ApprovalReq,
  type Msg,
  type WorkspaceContext,
  type WorkspaceMap,
  type WorkspaceState,
} from "./workspaceSessions";

let workspaces: WorkspaceMap = {};
const runContexts = new Map<string, string>();
const listeners = new Set<() => void>();

function emit() {
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function getSnapshot() {
  return workspaces;
}

export function useWorkspaces(): WorkspaceMap {
  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}

export function updateWorkspace(
  key: string,
  context: WorkspaceContext,
  updater: (workspace: WorkspaceState) => WorkspaceState,
) {
  workspaces = upsertWorkspace(workspaces, key, context, updater);
  emit();
}

export function registerRun(runId: string, key: string) {
  runContexts.set(runId, key);
}

export function updateRunWorkspace(
  runId: string | null | undefined,
  updater: (workspace: WorkspaceState) => WorkspaceState,
): boolean {
  const key = runId ? runContexts.get(runId) : undefined;
  if (!runId || !key) return false;
  const current = workspaces[key];
  if (!current) return false;
  updateWorkspace(key, current, updater);
  return true;
}

export function workspaceKeyForRun(runId: string | null | undefined): string | null {
  return runId ? runContexts.get(runId) ?? null : null;
}

export function workspaceContextForRun(runId: string | null | undefined): WorkspaceContext | null {
  const key = workspaceKeyForRun(runId);
  const workspace = key ? workspaces[key] : undefined;
  if (!workspace) return null;
  return {
    projectRoot: workspace.projectRoot,
    featureSlug: workspace.featureSlug,
    step: workspace.step,
  };
}

export function rekeyRunWorkspace(runId: string | null | undefined, featureSlug: string | null): boolean {
  const oldKey = runId ? runContexts.get(runId) : undefined;
  if (!runId || !oldKey) return false;
  const current = workspaces[oldKey];
  if (!current) return false;
  const context = normalizeWorkspaceContext({
    projectRoot: current.projectRoot,
    featureSlug,
    step: current.step,
  });
  const newKey = workspaceKey(context.projectRoot, context.featureSlug, context.step);
  if (newKey === oldKey) return true;
  const next = { ...current, ...context, key: newKey };
  workspaces = { ...workspaces };
  delete workspaces[oldKey];
  const existing = workspaces[newKey];
  workspaces[newKey] = existing
    ? {
        ...next,
        draft: existing.draft || next.draft,
        messages: [...existing.messages, ...next.messages],
        approvals: [...existing.approvals, ...next.approvals],
      }
    : next;
  runContexts.set(runId, newKey);
  emit();
  return true;
}

export function addRunMessage(runId: string, message: Msg) {
  updateRunWorkspace(runId, (workspace) => appendMessage(workspace, message));
}

export function removeApprovalEverywhere(approvalId: string) {
  const key = Object.keys(workspaces).find((candidate) =>
    workspaces[candidate].approvals.some((approval) => approval.approval_id === approvalId)
  );
  if (!key) return;
  updateWorkspace(key, workspaces[key], (workspace) => ({
    ...workspace,
    approvals: workspace.approvals.filter((approval) => approval.approval_id !== approvalId),
  }));
}

export type { ApprovalReq, Msg, WorkspaceContext, WorkspaceState };


// Development-only handle used by native WebKit inspector regression tests.
// Production builds eliminate this branch, so it is not a runtime API surface.
if (import.meta.env.DEV) {
  (window as unknown as { __AIX_WORKSPACE_DEV__?: unknown }).__AIX_WORKSPACE_DEV__ = {
    workspaceKey,
    registerRun,
    rekeyRunWorkspace,
    workspaceContextForRun,
    updateWorkspace,
    updateRunWorkspace,
  };
}
