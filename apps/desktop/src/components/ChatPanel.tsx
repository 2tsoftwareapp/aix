import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  ShieldQuestion, CheckCircle2, XCircle,
  Wrench, AlertTriangle, KeyRound, FolderOpen, Lock, CircleCheck, ChevronDown } from "lucide-react";
import { useKeyStatus, useVendorConfigs } from "../lib/providerKeys";
import { STEPS, STEP_ICONS, stepStatus } from "../lib/steps";
import { useStepGates, blockedTarget } from "../lib/stepGates";
import { useAppToast } from "../lib/toast";
import {
  sortCatalog, formatContextLength, vendorOf,
  type CatalogEntry,
} from "../lib/models";
import { HStack, VStack, StackItem } from "@astryxdesign/core/Layout";
import { Text } from "@astryxdesign/core/Text";
import { Icon } from "@astryxdesign/core/Icon";
import { Button } from "@astryxdesign/core/Button";
import { Badge } from "@astryxdesign/core/Badge";
import { Card } from "@astryxdesign/core/Card";
import { CodeBlock } from "@astryxdesign/core/CodeBlock";
import { EmptyState } from "@astryxdesign/core/EmptyState";
import { Spinner } from "@astryxdesign/core/Spinner";
import { Selector } from "@astryxdesign/core/Selector";
import { Markdown } from "@astryxdesign/core/Markdown";
import {
  ChatComposer,
  ChatComposerInput,
  ChatSendButton,
  ChatLayout,
  ChatMessage,
  ChatMessageBubble,
  ChatMessageList,
  ChatSystemMessage,
} from "@astryxdesign/core/Chat";
import type { StepId, PipelineState } from "../types";
import {
  appendMessage,
  appendStreamDelta,
  workspaceKey,
  type ApprovalReq,
  type Msg,
  type WorkspaceContext, type WorkspaceMap, stepWorkspaceMatches } from "../lib/workspaceSessions";
import {
  registerRun,
  rekeyRunWorkspace,
  removeApprovalEverywhere,
  updateRunWorkspace,
  updateWorkspace,
  useWorkspaces,
  workspaceContextForRun,
} from "../lib/workspaceStore";

const PROVIDER_LABELS: Record<string, string> = {
  anthropic: "Anthropic",
  openai: "OpenAI",
  openrouter: "OpenRouter",
  qwen: "Qwen",
};

const MODEL_OPTIONS: Record<string, string[]> = {
  anthropic: ["claude-opus-4-5", "claude-sonnet-4-5", "claude-haiku-4-5", "claude-sonnet-4", "claude-opus-4-1"],
  openai: ["gpt-5.2", "gpt-5.2-mini", "gpt-5.1", "gpt-5-mini", "o4-mini", "gpt-4.1"],
  openrouter: [
    "anthropic/claude-opus-4-5",
    "anthropic/claude-sonnet-4-5",
    "openai/gpt-5.2",
    "google/gemini-3-pro",
    "deepseek/deepseek-v3.2",
    "qwen/qwen3-max",
    "x-ai/grok-4.1",
  ],
  qwen: ["qwen3-max", "qwen3-plus", "qwen3-turbo", "qwen3-coder-plus", "qwen-max-latest", "qwen-plus-latest", "qwen-flash"],
};

function staticModelsFor(provider: string): string[] {
  return MODEL_OPTIONS[provider] ?? [];
}

const STEP_HINTS: Record<StepId, string> = {
  constitution: "Define project principles — evaluated against every later phase.",
  specify: "Create a new numbered feature describing what to build and why. After the run, the new feature and spec open automatically.",
  clarify: "Resolve ambiguity before planning. Up to 5 targeted questions per run.",
  plan: "Tech stack, architecture, and technical constraints.",
  checklist: "Unit tests for your requirements — reviewer-owned quality gates.",
  tasks: "Dependency-ordered implementation tasks, grouped by user story.",
  analyze: "Read-only cross-artifact consistency check.",
  implement: "Execute tasks phase by phase, respecting parallel markers.",
  converge: "Assess implementation against artifacts; loop until Converged.",
};

const STEP_STARTERS: Record<StepId, { title: string; description: string; prompts: string[] }> = {
  constitution: {
    title: "Draft the project constitution",
    description: "The constitution pins down principles every later phase is evaluated against — quality bars, prohibited shortcuts, review rules.",
    prompts: [
      "Draft a constitution for this project based on the codebase and existing docs",
      "We value correctness over speed — encode that with concrete, testable principles",
    ],
  },
  specify: {
    title: "Specify a new feature",
    description: "Creates a numbered feature spec: what to build, for whom, and why — no tech stack yet. The spec opens in the artifact panel when done.",
    prompts: [
      "Add a feature: ",
      "Spec out the next highest-value feature you can infer from the codebase",
    ],
  },
  clarify: {
    title: "Resolve ambiguity in the spec",
    description: "Reads spec.md and asks up to 5 targeted questions about anything unclear — answers get written back into the spec before planning starts.",
    prompts: [
      "Review spec.md and ask me about anything ambiguous or underspecified",
      "Focus on edge cases and error handling — quiz me on the risky parts",
    ],
  },
  plan: {
    title: "Write the technical plan",
    description: "Picks the tech stack, architecture, and constraints that satisfy the spec — this is where implementation decisions get made.",
    prompts: [
      "Write the implementation plan for this feature",
      "Plan it, preferring the simplest architecture that satisfies the spec",
    ],
  },
  checklist: {
    title: "Generate quality checklists",
    description: "Unit tests for the requirements themselves — reviewer-owned gates the agent cannot self-approve.",
    prompts: [
      "Generate checklists for the spec's requirements",
    ],
  },
  tasks: {
    title: "Break the plan into tasks",
    description: "Dependency-ordered implementation tasks grouped by user story, with parallel markers where safe.",
    prompts: [
      "Break the plan into implementation tasks",
    ],
  },
  analyze: {
    title: "Cross-check the artifacts",
    description: "Read-only consistency analysis across spec, plan, and tasks — catches contradictions before they reach code.",
    prompts: [
      "Analyze the artifacts for inconsistencies",
    ],
  },
  implement: {
    title: "Implement the tasks",
    description: "Executes tasks phase by phase, respecting dependencies and parallel markers.",
    prompts: [
      "Implement the tasks, phase by phase",
    ],
  },
  converge: {
    title: "Converge implementation with spec",
    description: "Assesses implementation against artifacts and loops until converged.",
    prompts: [
      "Assess the implementation against the spec and fix gaps",
    ],
  },
};

const APPROVAL_MODE_HINTS: Record<string, string> = {
  ask: "ask — every tool call requires approval",
  "auto-edit": "auto-edit — file edits auto-approved, shell still gated",
  "full-auto": "full-auto — no approval gates (use with care)",
};

type StreamPayload = { run_id?: string | null } & (
  | { kind: "run_context"; feature_slug: string | null }
  | { kind: "delta"; text: string }
  | { kind: "tool_use"; name: string }
  | { kind: "tool_result"; name: string; ok: boolean; detail: string }
  | { kind: "artifact_written"; path: string }
  | { kind: "gate_blocked"; reason: string }
  | { kind: "done"; feature_slug: string | null }
  | { kind: "error"; message: string }
);

function SystemLine({ msg }: { msg: Extract<Msg, { role: "system" }> }) {
  const icon =
    msg.variant === "success" ? CheckCircle2
    : msg.variant === "error" ? XCircle
    : msg.variant === "warning" ? AlertTriangle
    : msg.variant === "tool" ? Wrench
    : CheckCircle2;
  const color =
    msg.variant === "success" ? "success"
    : msg.variant === "error" ? "error"
    : msg.variant === "warning" ? "warning"
    : "secondary";
  return (
    <HStack gap={2} vAlign="start" width="100%" padding={1}>
      <Icon icon={icon} size="sm" color={color} />
      <Text type="code" size="xsm" color="secondary" style={{ textAlign: "left", flex: 1, minWidth: 0 }}>{msg.text}</Text>
    </HStack>
  );
}

function ApprovalCard({ req, onAnswer }: { req: ApprovalReq; onAnswer: (id: string, ok: boolean) => void }) {
  const isWrite = req.tool === "write_file";
  return (
    <Card width="100%" padding={3}>
      <VStack gap={3} width="100%">
        <HStack gap={2} vAlign="center">
          <Icon icon={ShieldQuestion} size="sm" color="accent" />
          <Text type="label" weight="semibold">Approve {req.tool}?</Text>
        </HStack>
        <Text type="code" size="xsm" color="secondary" display="block" wordBreak="break-all">
          {req.detail}
        </Text>
        {req.preview && (
          <VStack gap={1} width="100%">
            {isWrite && <Text type="supporting" color="secondary">Diff preview</Text>}
            <CodeBlock
              code={req.preview}
              language="plaintext"
              isDiff={isWrite}
              isWrapped
              width="100%"
              maxHeight={260}
              size="sm"
            />
          </VStack>
        )}
        <HStack gap={2}>
          <Button label="Approve" size="sm" variant="primary" onClick={() => onAnswer(req.approval_id, true)} />
          <Button label="Deny" size="sm" variant="secondary" onClick={() => onAnswer(req.approval_id, false)} />
        </HStack>
      </VStack>
    </Card>
  );
}

function ReadyState({
  projectRoot,
  providerId,
  providerHasKey,
  ready,
  step,
  featureSlug,
  hasConstitution,
  onOpenProject,
  onOpenSetup,
  onUsePrompt,
  onInitProject,
}: {
  projectRoot: string | null;
  providerId: string;
  providerHasKey: boolean;
  ready: boolean;
  step: StepId;
  featureSlug: string | null;
  hasConstitution: boolean;
  onOpenProject: () => Promise<void>;
  onOpenSetup: () => void;
  onUsePrompt: (prompt: string) => void;
  onInitProject: () => void;
}) {
  if (!projectRoot) {
    return (
      <EmptyState
        icon={<Icon icon={FolderOpen} size="lg" color="secondary" />}
        title="Open a project to begin"
        description="Choose a Spec Kit project folder. You can inspect its specs before connecting a provider."
        actions={<Button label="Open project…" variant="primary" icon={<Icon icon={FolderOpen} size="sm" />} clickAction={onOpenProject} />}
      />
    );
  }
  if (!providerHasKey) {
    return (
      <EmptyState
        icon={<Icon icon={KeyRound} size="lg" color="secondary" />}
        title={`Connect ${PROVIDER_LABELS[providerId] ?? providerId} to run agents`}
        description="You can browse the project now. Add an API key only when you are ready to run a pipeline step."
        actions={<Button label="Connect provider" variant="primary" icon={<Icon icon={KeyRound} size="sm" />} onClick={onOpenSetup} />}
      />
    );
  }
  if (ready) {
    if (!hasConstitution && step !== "constitution") {
      return (
        <EmptyState
          icon={<Icon icon={STEP_ICONS.constitution} size="lg" color="accent" />}
          title="Initialize Spec Kit for this project"
          description="This project has no speckit constitution yet. The constitution pins down the principles every later phase is evaluated against — start there, then specify your first feature."
          actions={
            <Button
              label="Draft constitution"
              variant="primary"
              icon={<Icon icon={STEP_ICONS.constitution} size="sm" />}
              onClick={onInitProject}
            />
          }
        />
      );
    }
    const starter = STEP_STARTERS[step];
    return (
      <VStack gap={4} hAlign="center" padding={4} width="100%" style={{ maxWidth: 560 }}>
        <VStack gap={2} hAlign="center">
          <Icon icon={STEP_ICONS[step]} size="lg" color="accent" />
          <Text type="large" weight="semibold">{starter.title}</Text>
          <Text type="body" color="secondary" style={{ textAlign: "center", maxWidth: 480 }}>
            {starter.description}
          </Text>
          {featureSlug && <Text type="code" size="xsm" color="secondary">{featureSlug}</Text>}
        </VStack>
        <VStack gap={2} width="100%">
          {starter.prompts.map((prompt) => (
            <button
              key={prompt}
              type="button"
              className="aix-starter-prompt"
              onClick={() => onUsePrompt(prompt)}
              style={{
                display: "block",
                width: "100%",
                textAlign: "left",
                padding: "10px 14px",
                borderRadius: "var(--radius-element)",
                border: "1px solid var(--color-border)",
                backgroundColor: "transparent",
                color: "var(--color-text-secondary)",
                fontSize: "var(--text-body-size)",
                cursor: "pointer",
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.borderColor = "var(--color-accent)";
                e.currentTarget.style.color = "var(--color-text-primary)";
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.borderColor = "var(--color-border)";
                e.currentTarget.style.color = "var(--color-text-secondary)";
              }}
            >
              {prompt}
            </button>
          ))}
        </VStack>
        <Text type="supporting" size="xsm" color="secondary">Enter to run · Shift+Enter for a newline</Text>
      </VStack>
    );
  }
  return null;
}

function StepNavigator({
  step,
  pipeline,
  projectRoot,
  featureSlug,
  workspaces,
  stepGates,
  onSelectStep,
}: {
  step: StepId;
  pipeline: PipelineState | null;
  projectRoot: string | null;
  featureSlug: string | null;
  workspaces: WorkspaceMap;
  stepGates: Partial<Record<StepId, { allowed: boolean; reason: string | null }>>;
  onSelectStep: (s: StepId) => void;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const stepIndex = STEPS.findIndex((s2) => s2.id === step);

  const chipStyle: React.CSSProperties = {
    display: "flex", alignItems: "center", gap: 6,
    padding: "5px 10px", borderRadius: 8, cursor: "pointer",
    background: "var(--color-background-muted, rgba(255,255,255,0.06))",
    border: "1px solid var(--color-border)",
    color: "inherit", font: "inherit", outline: "none",
  };

  return (
    <div ref={ref} style={{ position: "relative", flexShrink: 0 }}>
      {open && (
        <div
          className="aix-menu-pop"
          style={{
            position: "absolute", top: "calc(100% + 6px)", left: 0, zIndex: 30,
            width: 232, borderRadius: 12, overflow: "hidden",
            background: "var(--color-background-popover, #1c1c1e)",
            border: "1px solid var(--color-border)",
            boxShadow: "0 8px 28px rgba(0,0,0,0.45)",
          }}
          role="menu"
          aria-label="Pipeline steps"
        >
          {STEPS.map((s2, i) => {
            const gate = stepGates[s2.id];
            const blocked = gate != null && !gate.allowed;
            const target = blockedTarget(blocked ? gate?.reason : null);
            const status = stepStatus(s2.id, pipeline, featureSlug);
            const isCurrent = s2.id === step;
            const ws = projectRoot
              ? Object.values(workspaces).find((candidate) =>
                  stepWorkspaceMatches(candidate, projectRoot, featureSlug, s2.id)
                )
              : undefined;
            return (
              <div
                key={s2.id}
                role="menuitem"
                style={{
                  display: "flex", alignItems: "center", width: "100%",
                  background: isCurrent ? "rgba(255,255,255,0.08)" : "transparent",
                  opacity: blocked ? 0.65 : 1,
                }}
                onMouseEnter={(e) => { if (!isCurrent) e.currentTarget.style.background = "rgba(255,255,255,0.05)"; }}
                onMouseLeave={(e) => { if (!isCurrent) e.currentTarget.style.background = "transparent"; }}
              >
              <button
                type="button"
                title={blocked ? (gate?.reason ?? "blocked") : undefined}
                aria-label={blocked ? `${s2.label}: ${gate?.reason ?? "blocked"}` : s2.label}
                onClick={() => { onSelectStep(s2.id); setOpen(false); }}
                style={{
                  display: "flex", alignItems: "center", gap: 8, flex: 1, minWidth: 0,
                  padding: "8px 12px", border: "none", cursor: "pointer",
                  background: "transparent",
                  color: "inherit", font: "inherit", textAlign: "left",
                }}
              >
                {blocked ? (
                  <Icon icon={Lock} size="xsm" color="secondary" />
                ) : status === "done" ? (
                  <Icon icon={STEP_ICONS[s2.id]} size="xsm" color="accent" />
                ) : (
                  <Icon icon={STEP_ICONS[s2.id]} size="xsm" color={isCurrent ? "accent" : "secondary"} />
                )}
                <span style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  <Text type="supporting" weight={isCurrent ? "semibold" : undefined} color={blocked ? "secondary" : undefined} maxLines={1}>
                    {s2.label}
                  </Text>
                </span>
                {status === "done" && <Icon icon={CircleCheck} size="xsm" color="accent" />}
                {ws?.running && <Spinner size="sm" shade="subtle" aria-label="running" />}
                {ws != null && ws.approvals.length > 0 && <Icon icon={ShieldQuestion} size="xsm" color="warning" aria-label="approval pending" />}
                <Text type="code" size="2xs" color="secondary">{i + 1}</Text>
              </button>
                {target && target !== s2.id && (
                  <button
                    type="button"
                    aria-label={`Go to ${target}`}
                    title={`Missing prerequisite — go to ${STEPS.find((x) => x.id === target)?.label ?? target}`}
                    onClick={() => { onSelectStep(target); setOpen(false); }}
                    style={{
                      background: "none", border: "none", padding: "0 10px 0 0",
                      color: "var(--color-accent, #4d9fff)", fontSize: 11, fontFamily: "inherit",
                      whiteSpace: "nowrap", cursor: "pointer",
                    }}
                  >
                    → {STEPS.find((x) => x.id === target)?.label ?? target}
                  </button>
                )}
              </div>
            );
          })}
        </div>
      )}
      <button
        type="button"
        style={{ ...chipStyle, background: open ? "var(--color-background-muted, rgba(255,255,255,0.06))" : "transparent" }}
        onClick={() => setOpen((v) => !v)}
        aria-label="Open pipeline steps"
        aria-haspopup="menu"
        aria-expanded={open}
      >
        <Icon icon={STEP_ICONS[step]} size="xsm" color="accent" />
        <Text type="supporting" weight="medium">{STEPS[stepIndex]?.label ?? step}</Text>
        <Text type="code" size="2xs" color="secondary">{stepIndex + 1}/9</Text>
        <Icon icon={ChevronDown} size="xsm" color="secondary" />
      </button>
    </div>
  );
}

export function ChatPanel({
  step,
  pipeline,
  projectRoot,
  featureSlug,
  onSelectStep,
  onStepDone,
  onOpenProject,
  onOpenSetup,
}: {
  step: StepId;
  pipeline: PipelineState | null;
  projectRoot: string | null;
  featureSlug: string | null;
  onSelectStep: (s: StepId) => void;
  onStepDone: (context: WorkspaceContext | null, completedFeatureSlug: string | null) => void;
  onOpenProject: () => Promise<void>;
  onOpenSetup: () => void;
}) {
  const toast = useAppToast();
  const workspaces = useWorkspaces();
  const stepGates = useStepGates(projectRoot, featureSlug, pipeline);
  const [providerId, setProviderId] = useState(() => localStorage.getItem("aix-provider") ?? "anthropic");
  const [model, setModel] = useState(() => localStorage.getItem("aix-model") ?? "claude-sonnet-4-5");
  const [approvalMode, setApprovalMode] = useState(() => localStorage.getItem("aix-approval-mode") ?? "ask");
  const keyStatus = useKeyStatus();
  const vendorConfigs = useVendorConfigs();
  const providerHasKey = !!keyStatus[providerId];
  const [gate, setGate] = useState<{ allowed: boolean; needs_confirmation: boolean; reason: string | null } | null>(null);
  const [liveModels, setLiveModels] = useState<Record<string, string[]>>({});
  const [catalog, setCatalog] = useState<CatalogEntry[]>([]);
  const context: WorkspaceContext | null = projectRoot ? { projectRoot, featureSlug, step } : null;
  const key = context ? workspaceKey(context.projectRoot, context.featureSlug, context.step) : null;
  const workspace = key ? workspaces[key] : undefined;
  const input = workspace?.draft ?? "";
  const messages = workspace?.messages ?? [];
  const approvals = workspace?.approvals ?? [];
  const running = workspace?.running ?? false;
  const activeRunId = workspace?.activeRunId ?? null;
  const failedPrompt = workspace?.failedPrompt ?? null;
  const anyRunning = Object.values(workspaces).some((candidate) => candidate.running);
  const otherRun = Object.values(workspaces).find((candidate) => candidate.running && candidate.key !== key);
  const currentContextRef = useRef<{ key: string | null; context: WorkspaceContext | null }>({ key: null, context: null });

  useEffect(() => {
    currentContextRef.current = { key, context };
  }, [key, context]);

  const setInput = (value: string) => {
    if (!key || !context) return;
    updateWorkspace(key, context, (current) => ({ ...current, draft: value }));
  };

  const applyToRoutedWorkspace = (
    runId: string | null | undefined,
    updater: Parameters<typeof updateWorkspace>[2],
  ) => {
    if (updateRunWorkspace(runId, updater)) return;
    const current = currentContextRef.current;
    if (current.key && current.context) updateWorkspace(current.key, current.context, updater);
  };

  useEffect(() => {
    const CACHE_KEY = "aix-openrouter-catalog-v2";
    const derive = (ids: string[]): Record<string, string[]> => {
      const strip = (vendor: string) =>
        ids
          .filter((id) => id.startsWith(vendor + "/") && !id.includes(":"))
          .map((id) => id.slice(vendor.length + 1));
      return {
        anthropic: strip("anthropic").map((id) => id.replace(/\./g, "-")),
        openai: strip("openai").filter((id) => /^(gpt-|o\d|chatgpt-)/.test(id)),
        qwen: strip("qwen").filter((id) => id.startsWith("qwen")),
      };
    };
    const apply = (entries: CatalogEntry[]) => {
      const usable = entries.filter((e) => !e.id.includes(":"));
      setCatalog(sortCatalog(usable));
      setLiveModels((m) => ({ ...m, ...derive(usable.map((e) => e.id)) }));
    };
    try {
      const cached = JSON.parse(localStorage.getItem(CACHE_KEY) ?? "null");
      if (cached && Date.now() - cached.at < 24 * 3600 * 1000 && cached.entries?.length > 0) {
        apply(cached.entries);
        return;
      }
    } catch { /* fall through to fetch */ }
    invoke<CatalogEntry[]>("list_models", { providerId: "openrouter" })
      .then((models) => {
        if (models.length === 0) return;
        localStorage.setItem(CACHE_KEY, JSON.stringify({ at: Date.now(), entries: models }));
        apply(models);
      })
      .catch(() => { /* static fallback lists remain */ });
  }, []);

  // live catalog for direct vendors — OpenAI-compatible vendors expose /models;
  // cached per vendor for an hour so the picker doesn't spam the API
  useEffect(() => {
    if (providerId === "openrouter" || !keyStatus[providerId]) return;
    const cacheKey = `aix-vendor-models-${providerId}`;
    const applyIds = (ids: string[]) => {
      if (ids.length > 0) setLiveModels((m) => ({ ...m, [providerId]: ids }));
    };
    try {
      const cached = JSON.parse(localStorage.getItem(cacheKey) ?? "null");
      if (cached && Date.now() - cached.at < 3600 * 1000 && cached.ids.length > 0) {
        applyIds(cached.ids);
        return;
      }
    } catch { /* refetch */ }
    invoke<{ id: string }[]>("list_models", { providerId })
      .then((models) => {
        const ids = models.map((m) => m.id);
        localStorage.setItem(cacheKey, JSON.stringify({ at: Date.now(), ids }));
        applyIds(ids);
      })
      .catch(() => { /* static fallback or manual entry */ });
  }, [providerId, keyStatus]);

  useEffect(() => { localStorage.setItem("aix-provider", providerId); }, [providerId]);
  useEffect(() => { localStorage.setItem("aix-model", model); }, [model]);
  useEffect(() => { localStorage.setItem("aix-approval-mode", approvalMode); }, [approvalMode]);

  useEffect(() => {
    if (!projectRoot) return;
    invoke<{ allowed: boolean; needs_confirmation: boolean; reason: string | null }>("check_gate", {
      step,
      projectRoot,
      featureSlug,
    }).then(setGate).catch(() => setGate(null));
  }, [step, projectRoot, featureSlug, running]);

  useEffect(() => {
    if (!keyStatus[providerId]) {
      const configured = Object.keys(keyStatus).find((p) => keyStatus[p]);
      if (configured) {
        setProviderId(configured);
        setModel(staticModelsFor(configured)[0] ?? "");
        toast.info(`Switched to ${configured} — the configured provider`);
      }
    }
  }, [keyStatus, providerId, toast]);

  useEffect(() => {
    const unlisten = listen<StreamPayload>("step-stream", (ev) => {
      const p = ev.payload;
      if (p.kind === "run_context") {
        rekeyRunWorkspace(p.run_id, p.feature_slug);
      } else if (p.kind === "delta") {
        applyToRoutedWorkspace(p.run_id, (current) => appendStreamDelta(current, p.text));
      } else if (p.kind === "tool_result") {
        applyToRoutedWorkspace(p.run_id, (current) => appendMessage(current, {
          role: "system",
          variant: p.ok ? "tool" : "error",
          text: `${p.name} — ${p.detail}`,
        }));
      } else if (p.kind === "artifact_written") {
        applyToRoutedWorkspace(p.run_id, (current) => appendMessage(current, {
          role: "system",
          variant: "success",
          text: `artifact written: ${p.path}`,
        }));
      } else if (p.kind === "gate_blocked") {
        applyToRoutedWorkspace(p.run_id, (current) => ({
          ...appendMessage(current, { role: "system", variant: "error", text: `gate blocked: ${p.reason}` }),
          running: false,
          activeRunId: null,
        }));
      } else if (p.kind === "error") {
        applyToRoutedWorkspace(p.run_id, (current) => ({
          ...appendMessage(current, { role: "system", variant: "error", text: `error: ${p.message}` }),
          approvals: [],
          running: false,
          activeRunId: null,
          failedPrompt: current.lastPrompt,
        }));
      } else if (p.kind === "done") {
        const doneContext = workspaceContextForRun(p.run_id);
        applyToRoutedWorkspace(p.run_id, (current) => ({
          ...current,
          approvals: [],
          running: false,
          activeRunId: null,
        }));
        onStepDone(doneContext, p.feature_slug);
      }
    });
    return () => { unlisten.then((f) => f()); };
  }, [onStepDone]);

  useEffect(() => {
    const unlisten = listen<{
      kind: string; approval_id: string; tool: string; detail: string; preview: string | null; run_id?: string | null;
    }>("approval-request", (ev) => {
      if (ev.payload.kind !== "request") return;
      applyToRoutedWorkspace(ev.payload.run_id, (current) => ({
        ...current,
        approvals: [
          ...current.approvals,
          {
            approval_id: ev.payload.approval_id,
            tool: ev.payload.tool,
            detail: ev.payload.detail,
            preview: ev.payload.preview,
            step: current.step,
            projectRoot: current.projectRoot,
            featureSlug: current.featureSlug,
            runId: ev.payload.run_id ?? undefined,
          },
        ],
      }));
    });
    return () => { unlisten.then((f) => f()); };
  }, []);

  const answerApproval = async (id: string, approved: boolean) => {
    const request = approvals.find((approval) => approval.approval_id === id);
    removeApprovalEverywhere(id);
    try {
      await invoke("respond_approval", { approvalId: id, approved });
    } catch (e) {
      applyToRoutedWorkspace(request?.runId, (current) => appendMessage(current, {
        role: "system",
        variant: "error",
        text: `approval response failed: ${e}`,
      }));
    }
  };

  const run = async (submitted = input) => {
    if (!projectRoot || !context || !key || !submitted.trim() || (gate && !gate.allowed) || running || anyRunning) return;
    const userInput = submitted;
    const runId = crypto.randomUUID();
    registerRun(runId, key);
    updateWorkspace(key, context, (current) => ({
      ...current,
      draft: "",
      messages: [...current.messages, { role: "user", text: userInput }],
      running: true,
      activeRunId: runId,
      failedPrompt: null,
      lastPrompt: userInput,
    }));
    try {
      await invoke("run_step", {
        input: {
          step,
          project_root: projectRoot,
          feature_slug: featureSlug,
          user_input: userInput,
          provider_id: providerId,
          model,
          approval_mode: approvalMode,
          confirmed: gate?.needs_confirmation ? true : undefined,
          run_id: runId,
        },
      });
    } catch (e) {
      updateRunWorkspace(runId, (current) => ({
        ...appendMessage(current, { role: "system", variant: "error", text: `invoke failed: ${e}` }),
        running: false,
        activeRunId: null,
        failedPrompt: userInput,
      }));
    }
  };

  const ready = projectRoot && providerHasKey;
  const gateBlocked = gate != null && !gate.allowed;
  const modelIds = useMemo(() => {
    const statics = staticModelsFor(providerId);
    return liveModels[providerId]?.length
      ? [...statics.filter((m) => liveModels[providerId].includes(m)),
         ...liveModels[providerId].filter((m) => !statics.includes(m))]
      : statics;
  }, [liveModels, providerId]);

  const openrouterEntries = useMemo<CatalogEntry[]>(() => (
    catalog.length > 0 ? catalog : MODEL_OPTIONS.openrouter.map((id) => ({ id }))
  ), [catalog]);

  const modelOptions = useMemo(() => {
    const items = providerId === "openrouter"
      ? openrouterEntries.map((entry) => ({
          value: entry.id,
          label: entry.name || entry.id,
          description: [vendorOf(entry.id), formatContextLength(entry.context_length)]
            .filter(Boolean).join(" · ") || undefined,
        }))
      : modelIds.map((id) => ({ value: id, label: id }));
    if (model && !items.some((item) => item.value === model)) {
      return [{ value: model, label: model, description: "custom" }, ...items];
    }
    return items;
  }, [providerId, openrouterEntries, modelIds, model]);

  const hiddenApprovalWorkspace = Object.values(workspaces).find((candidate) =>
    candidate.key !== key
    && candidate.projectRoot === projectRoot
    && candidate.approvals.length > 0
  );

  const messageNodes = [
    ...messages.map((m, i) => {
      const isLastAssistant = m.role === "assistant" && i === messages.length - 1;
      if (m.role === "system") return <SystemLine key={i} msg={m} />;
      if (m.role === "user") {
        return (
          <ChatMessage key={i} sender="user" name="you">
            <ChatMessageBubble>
              <Markdown density="compact">{m.text}</Markdown>
            </ChatMessageBubble>
          </ChatMessage>
        );
      }
      return (
        <ChatMessage key={i} sender="assistant" name="assistant">
          <ChatMessageBubble variant="ghost">
            <Markdown density="compact" isStreaming={isLastAssistant && running}>{m.text}</Markdown>
            {isLastAssistant && running && <Text color="accent">▍</Text>}
          </ChatMessageBubble>
        </ChatMessage>
      );
    }),
    ...(running && messages[messages.length - 1]?.role !== "assistant" ? [
      <ChatSystemMessage key="thinking" icon={<Spinner size="sm" shade="subtle" aria-label="Thinking" />}>
        <Text type="supporting" color="secondary">thinking…</Text>
      </ChatSystemMessage>,
    ] : []),
    ...approvals.map((a) => (
      <ApprovalCard key={a.approval_id} req={a} onAnswer={answerApproval} />
    )),
    ...(hiddenApprovalWorkspace ? [
      <ChatSystemMessage key="hidden-approval" icon={<Icon icon={ShieldQuestion} size="sm" color="warning" />}>
        <Text type="supporting" color="secondary">
          Approval pending in {hiddenApprovalWorkspace.step}{hiddenApprovalWorkspace.featureSlug ? ` · ${hiddenApprovalWorkspace.featureSlug}` : ""}; select that step in the sidebar to review it.
        </Text>
      </ChatSystemMessage>,
    ] : []),
  ];

  const composerStatus = gateBlocked
    ? { type: "error" as const, message: `${step} is blocked — ${gate.reason}` }
    : gate?.allowed && gate.needs_confirmation
      ? { type: "warning" as const, message: gate.reason ?? "Confirmation required" }
      : otherRun
        ? { type: "warning" as const, message: `${otherRun.step} is running; output is kept with that step.` }
        : failedPrompt
          ? { type: "warning" as const, message: "Last run failed — the prompt can be restored below." }
          : undefined;

  const stepIndex = STEPS.findIndex((s2) => s2.id === step);

  return (
    <VStack height="100%" width="100%" gap={0} style={{ position: "relative" }}>
      <VStack gap={0} width="100%" style={{ borderBottom: "1px solid var(--color-border)" }}>
        <HStack padding={3} vAlign="center" gap={3} width="100%">
          <HStack gap={2} vAlign="center" style={{ minWidth: 0 }}>
            <StepNavigator
              step={step}
              pipeline={pipeline}
              projectRoot={projectRoot}
              featureSlug={featureSlug}
              workspaces={workspaces}
              stepGates={stepGates}
              onSelectStep={onSelectStep}
            />
            {featureSlug && <Text type="supporting" color="secondary" maxLines={1}>{featureSlug}</Text>}
          </HStack>
          <StackItem size="fill" style={{ minWidth: 0 }}>
            <Text type="supporting" color="secondary" maxLines={1}>{STEP_HINTS[step]}</Text>
          </StackItem>
          {featureSlug && <Badge variant="neutral" label={featureSlug} />}
        </HStack>
      </VStack>
      <div key={step} className="aix-step-content" style={{ flex: 1, minHeight: 0, display: "flex", flexDirection: "column" }}>
      <ChatLayout
        density="compact"
          emptyState={
            <ReadyState
              projectRoot={projectRoot}
              providerId={providerId}
              providerHasKey={providerHasKey}
              ready={!!ready}
              step={step}
              featureSlug={featureSlug}
              hasConstitution={pipeline?.has_constitution ?? true}
              onOpenProject={onOpenProject}
              onOpenSetup={onOpenSetup}
              onUsePrompt={setInput}
              onInitProject={() => {
                if (!projectRoot) return;
                const targetContext: WorkspaceContext = { projectRoot, featureSlug, step: "constitution" };
                updateWorkspace(workspaceKey(projectRoot, featureSlug, "constitution"), targetContext, (current) => ({
                  ...current,
                  draft: "Draft a constitution for this project based on the codebase and existing docs",
                }));
                onSelectStep("constitution");
              }}
            />
          }
          composer={
            <ChatComposer
              value={input}
              onChange={setInput}
              onSubmit={(value) => void run(value)}
              placeholder={
                gateBlocked
                  ? `${step} is blocked — resolve the prerequisite first`
                  : ready
                    ? `Describe the input for /speckit.${step}…`
                    : !projectRoot
                      ? "Open a project to begin"
                      : `Connect ${PROVIDER_LABELS[providerId] ?? providerId} to run agents`
              }
              isDisabled={!ready || gateBlocked}
              input={<ChatComposerInput style={{ minHeight: 66 }} />}
              style={{ "--focus-outline-color": "transparent" } as React.CSSProperties}
              density="compact"
              elevation="none"
              status={composerStatus}
              footerActions={
                <HStack gap={1} vAlign="center">
                  {failedPrompt && (
                    <Button
                      label="Restore prompt"
                      variant="ghost"
                      size="sm"
                      onClick={() => {
                        if (!key || !context) return;
                        updateWorkspace(key, context, (current) => ({
                          ...current,
                          draft: failedPrompt,
                          failedPrompt: null,
                        }));
                      }}
                    />
                  )}
                  <Selector
                    label="Approval mode"
                    isLabelHidden
                    aria-label="Approval mode"
                    options={[
                      { value: "ask", label: "Ask first" },
                      { value: "auto-edit", label: "Auto edit" },
                      { value: "full-auto", label: "Full access" },
                    ]}
                    value={approvalMode}
                    onChange={setApprovalMode}
                    labelTooltip={APPROVAL_MODE_HINTS[approvalMode]}
                    size="sm"
                    variant="ghost"
                  />
                  <Selector
                    label="Provider"
                    isLabelHidden
                    aria-label="Provider"
                    options={(vendorConfigs.length > 0
                      ? vendorConfigs.map((v) => ({ id: v.id, display: v.display }))
                      : Object.keys(PROVIDER_LABELS).map((id) => ({ id, display: PROVIDER_LABELS[id] }))
                    ).map((v) => ({
                      value: v.id,
                      label: v.display,
                      description: keyStatus[v.id] ? "configured" : "no key",
                    }))}
                    value={providerId}
                    onChange={(p) => { setProviderId(p); setModel(staticModelsFor(p)[0] ?? ""); }}
                    size="sm"
                    variant="ghost"
                  />
                </HStack>
              }
              sendActions={
                <Selector
                  label="Model"
                  isLabelHidden
                  aria-label="Model"
                  options={modelOptions}
                  value={model}
                  onChange={(m) => { if (m) setModel(m); }}
                  size="sm"
                  variant="ghost"
                  hasSearch
                  searchPlaceholder="Search models…"
                  emptySearchText="No model matches"
                />
              }
              sendButton={
                projectRoot && !providerHasKey && !running ? (
                  <Button
                    label="Connect provider"
                    variant="primary"
                    size="md"
                    icon={<Icon icon={KeyRound} size="sm" />}
                    onClick={onOpenSetup}
                  />
                ) : (
                  <ChatSendButton
                    isStopShown={running}
                    isDisabled={!running && (!ready || !input.trim() || gateBlocked || anyRunning)}
                    onSend={() => void run()}
                    onStop={() => invoke("cancel_run", { runId: activeRunId }).catch(() => {})}
                    size="md"
                  />
                )
              }
            />
          }
        >
          {messageNodes.length > 0 && (
            <ChatMessageList align="bottom" isStreaming={running}>
              {messageNodes}
            </ChatMessageList>
          )}
        </ChatLayout>
      </div>
    </VStack>
  );
}
