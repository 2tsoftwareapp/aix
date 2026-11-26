import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  Send, Square, ShieldQuestion, FileText, CheckCircle2, XCircle,
  Wrench, AlertTriangle, Sprout, KeyRound, FolderOpen,
} from "lucide-react";
import { useKeyStatus } from "../lib/providerKeys";
import { useAppToast } from "../lib/toast";
import { searchModelItems, type ModelSearchItem } from "../lib/models";
import { HStack, VStack, StackItem } from "@astryxdesign/core/Layout";
import { Text, Heading } from "@astryxdesign/core/Text";
import { Icon } from "@astryxdesign/core/Icon";
import { Button } from "@astryxdesign/core/Button";
import { Badge } from "@astryxdesign/core/Badge";
import { Card } from "@astryxdesign/core/Card";
import { CodeBlock } from "@astryxdesign/core/CodeBlock";
import { EmptyState } from "@astryxdesign/core/EmptyState";
import { Spinner } from "@astryxdesign/core/Spinner";
import { Selector } from "@astryxdesign/core/Selector";
import { Typeahead, type SearchSource } from "@astryxdesign/core/Typeahead";
import { Markdown } from "@astryxdesign/core/Markdown";
import {
  ChatComposer,
  ChatLayout,
  ChatMessage,
  ChatMessageBubble,
  ChatMessageList,
  ChatSystemMessage,
} from "@astryxdesign/core/Chat";
import type { StepId } from "../types";

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

const STEP_HINTS: Record<StepId, string> = {
  constitution: "Define project principles — evaluated against every later phase.",
  specify: "Describe what to build and why. No tech stack here — that belongs in Plan.",
  clarify: "Resolve ambiguity before planning. Up to 5 targeted questions per run.",
  plan: "Tech stack, architecture, and technical constraints.",
  checklist: "Unit tests for your requirements — reviewer-owned quality gates.",
  tasks: "Dependency-ordered implementation tasks, grouped by user story.",
  analyze: "Read-only cross-artifact consistency check.",
  implement: "Execute tasks phase by phase, respecting parallel markers.",
  converge: "Assess implementation against artifacts; loop until Converged.",
};

const APPROVAL_MODE_HINTS: Record<string, string> = {
  ask: "ask — every tool call requires approval",
  "auto-edit": "auto-edit — file edits auto-approved, shell still gated",
  "full-auto": "full-auto — no approval gates (use with care)",
};

type StreamPayload =
  | { kind: "delta"; text: string }
  | { kind: "tool_use"; name: string }
  | { kind: "tool_result"; name: string; ok: boolean; detail: string }
  | { kind: "artifact_written"; path: string }
  | { kind: "gate_blocked"; reason: string }
  | { kind: "done" }
  | { kind: "error"; message: string };

interface Msg {
  role: "user" | "assistant" | "system";
  text: string;
  variant?: "info" | "success" | "error" | "warning" | "tool";
}

interface ApprovalReq {
  approval_id: string;
  tool: string;
  detail: string;
  preview: string | null;
  step: StepId;
  projectRoot: string | null;
  featureSlug: string | null;
}

function SystemLine({ msg }: { msg: Msg }) {
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
    <ChatSystemMessage icon={<Icon icon={icon} size="sm" color={color} />}>
      <Text type="code" size="xsm" color="secondary">{msg.text}</Text>
    </ChatSystemMessage>
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
  onOpenProject,
  onOpenSetup,
}: {
  projectRoot: string | null;
  providerId: string;
  providerHasKey: boolean;
  ready: boolean;
  step: StepId;
  onOpenProject: () => Promise<void>;
  onOpenSetup: () => void;
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
    return (
      <EmptyState
        icon={<Icon icon={Sprout} size="lg" color="accent" />}
        title={`Ready for ${step}`}
        description={`Describe the input for /speckit.${step} in the composer below. Enter to run, Shift+Enter for a newline.`}
      />
    );
  }
  return null;
}

export function ChatPanel({
  step,
  projectRoot,
  featureSlug,
  onStepDone,
  onOpenProject,
  onOpenSetup,
}: {
  step: StepId;
  projectRoot: string | null;
  featureSlug: string | null;
  onStepDone: () => void;
  onOpenProject: () => Promise<void>;
  onOpenSetup: () => void;
}) {
  const toast = useAppToast();
  const [input, setInput] = useState("");
  const [messages, setMessages] = useState<Msg[]>([]);
  const [running, setRunning] = useState(false);
  const [providerId, setProviderId] = useState(() => localStorage.getItem("aix-provider") ?? "anthropic");
  const [model, setModel] = useState(() => localStorage.getItem("aix-model") ?? "claude-sonnet-4-5");
  const [approvalMode, setApprovalMode] = useState(() => localStorage.getItem("aix-approval-mode") ?? "ask");
  const keyStatus = useKeyStatus();
  const providerHasKey = !!keyStatus[providerId];
  const [gate, setGate] = useState<{ allowed: boolean; needs_confirmation: boolean; reason: string | null } | null>(null);
  const [liveModels, setLiveModels] = useState<Record<string, string[]>>({});
  const [approvals, setApprovals] = useState<ApprovalReq[]>([]);
  const modelQueryRef = useRef("");
  const approvalContextRef = useRef({ step, projectRoot, featureSlug });

  useEffect(() => {
    approvalContextRef.current = { step, projectRoot, featureSlug };
  }, [step, projectRoot, featureSlug]);

  useEffect(() => {
    const CACHE_KEY = "aix-openrouter-catalog";
    const derive = (catalog: string[]): Record<string, string[]> => {
      const strip = (vendor: string) =>
        catalog
          .filter((id) => id.startsWith(vendor + "/") && !id.includes(":"))
          .map((id) => id.slice(vendor.length + 1));
      return {
        anthropic: strip("anthropic").map((id) => id.replace(/\./g, "-")),
        openai: strip("openai").filter((id) => /^(gpt-|o\d|chatgpt-)/.test(id)),
        qwen: strip("qwen").filter((id) => id.startsWith("qwen")),
        openrouter: catalog.filter((id) => !id.includes(":")),
      };
    };
    const apply = (catalog: string[]) => {
      const derived = derive(catalog);
      setLiveModels((m) => ({ ...m, ...derived }));
    };
    try {
      const cached = JSON.parse(localStorage.getItem(CACHE_KEY) ?? "null");
      if (cached && Date.now() - cached.at < 24 * 3600 * 1000 && cached.ids.length > 0) {
        apply(cached.ids);
        return;
      }
    } catch { /* fall through to fetch */ }
    invoke<{ id: string }[]>("list_models", { providerId: "openrouter" })
      .then((models) => {
        if (models.length === 0) return;
        const ids = models.map((m) => m.id);
        localStorage.setItem(CACHE_KEY, JSON.stringify({ at: Date.now(), ids }));
        apply(ids);
      })
      .catch(() => { /* static fallback lists remain */ });
  }, []);

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
      const first = ["anthropic", "openai", "openrouter", "qwen"].find((p) => keyStatus[p]);
      if (first) {
        setProviderId(first);
        setModel(MODEL_OPTIONS[first][0]);
        toast.info(`Switched to ${first} — the configured provider`);
      }
    }
  }, [keyStatus, providerId, toast]);

  useEffect(() => {
    setMessages([]);
  }, [step, projectRoot, featureSlug]);

  useEffect(() => {
    const unlisten = listen<StreamPayload>("step-stream", (ev) => {
      const p = ev.payload;
      if (p.kind === "delta") {
        setMessages((prev) => {
          const next = [...prev];
          const last = next[next.length - 1];
          if (last?.role === "assistant") {
            next[next.length - 1] = { ...last, text: last.text + p.text };
          } else {
            next.push({ role: "assistant", text: p.text });
          }
          return next;
        });
      } else if (p.kind === "tool_result") {
        setMessages((prev) => [
          ...prev,
          { role: "system", variant: p.ok ? "tool" : "error", text: `${p.name} — ${p.detail}` },
        ]);
      } else if (p.kind === "artifact_written") {
        setMessages((prev) => [...prev, { role: "system", variant: "success", text: `artifact written: ${p.path}` }]);
      } else if (p.kind === "gate_blocked") {
        setMessages((prev) => [...prev, { role: "system", variant: "error", text: `gate blocked: ${p.reason}` }]);
        setRunning(false);
      } else if (p.kind === "error") {
        setMessages((prev) => [...prev, { role: "system", variant: "error", text: `error: ${p.message}` }]);
        setApprovals([]);
        setRunning(false);
      } else if (p.kind === "done") {
        setApprovals([]);
        setRunning(false);
        onStepDone();
      }
    });
    return () => { unlisten.then((f) => f()); };
  }, [onStepDone]);

  useEffect(() => {
    const unlisten = listen<{
      kind: string; approval_id: string; tool: string; detail: string; preview: string | null;
    }>("approval-request", (ev) => {
      if (ev.payload.kind === "request") {
        const context = approvalContextRef.current;
        setApprovals((prev) => [
          ...prev,
          {
            approval_id: ev.payload.approval_id,
            tool: ev.payload.tool,
            detail: ev.payload.detail,
            preview: ev.payload.preview,
            step: context.step,
            projectRoot: context.projectRoot,
            featureSlug: context.featureSlug,
          },
        ]);
      }
    });
    return () => { unlisten.then((f) => f()); };
  }, []);

  const answerApproval = async (id: string, approved: boolean) => {
    setApprovals((prev) => prev.filter((a) => a.approval_id !== id));
    try {
      await invoke("respond_approval", { approvalId: id, approved });
    } catch (e) {
      setMessages((prev) => [...prev, { role: "system", variant: "error", text: `approval response failed: ${e}` }]);
    }
  };

  const run = async (submitted = input) => {
    if (!projectRoot || !submitted.trim() || (gate && !gate.allowed) || running) return;
    setRunning(true);
    setMessages((prev) => [...prev, { role: "user", text: submitted }]);
    const userInput = submitted;
    setInput("");
    const typedModel = modelQueryRef.current.trim();
    const effectiveModel = typedModel || model;
    if (typedModel && typedModel !== model) setModel(typedModel);
    try {
      await invoke("run_step", {
        input: {
          step,
          project_root: projectRoot,
          feature_slug: featureSlug,
          user_input: userInput,
          provider_id: providerId,
          model: effectiveModel,
          approval_mode: approvalMode,
          confirmed: gate?.needs_confirmation ? true : undefined,
        },
      });
    } catch (e) {
      setMessages((prev) => [...prev, { role: "system", variant: "error", text: `invoke failed: ${e}` }]);
      setRunning(false);
    }
  };

  const ready = projectRoot && providerHasKey;
  const gateBlocked = gate != null && !gate.allowed;
  const modelIds = useMemo(() => (
    liveModels[providerId]?.length
      ? [...MODEL_OPTIONS[providerId].filter((m) => liveModels[providerId].includes(m)),
         ...liveModels[providerId].filter((m) => !MODEL_OPTIONS[providerId].includes(m))]
      : MODEL_OPTIONS[providerId]
  ), [liveModels, providerId]);

  const modelSearchSource = useMemo<SearchSource<ModelSearchItem>>(() => ({
    search: (query: string) => searchModelItems(modelIds, query),
    bootstrap: () => searchModelItems(modelIds, ""),
  }), [modelIds]);
  const selectedModel = useMemo<ModelSearchItem>(() => ({ id: model, label: model }), [model]);

  const visibleApprovals = approvals.filter(
    (a) => a.step === step && a.projectRoot === projectRoot && a.featureSlug === featureSlug,
  );
  const hiddenApproval = approvals.find(
    (a) => !(a.step === step && a.projectRoot === projectRoot && a.featureSlug === featureSlug),
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
    ...visibleApprovals.map((a) => (
      <ApprovalCard key={a.approval_id} req={a} onAnswer={answerApproval} />
    )),
    ...(hiddenApproval ? [
      <ChatSystemMessage key="hidden-approval" icon={<Icon icon={ShieldQuestion} size="sm" color="warning" />}>
        <Text type="supporting" color="secondary">
          Approval pending in {hiddenApproval.step}; return to that step to review it.
        </Text>
      </ChatSystemMessage>,
    ] : []),
  ];

  const composerStatus = gateBlocked
    ? { type: "error" as const, message: `${step} is blocked — ${gate.reason}` }
    : gate?.allowed && gate.needs_confirmation
      ? { type: "warning" as const, message: gate.reason ?? "Confirmation required" }
      : undefined;

  return (
    <VStack height="100%" width="100%" gap={0}>
      <HStack padding={3} vAlign="center" justify="between" width="100%">
        <StackItem size="fill">
          <VStack gap={0}>
            <Heading level={1}>{step}</Heading>
            <Text type="supporting" color="secondary" maxLines={1}>{STEP_HINTS[step]}</Text>
          </VStack>
        </StackItem>
        {featureSlug && <Badge variant="neutral" label={featureSlug} />}
      </HStack>
      <ChatLayout
        density="compact"
          emptyState={
            <ReadyState
              projectRoot={projectRoot}
              providerId={providerId}
              providerHasKey={providerHasKey}
              ready={!!ready}
              step={step}
              onOpenProject={onOpenProject}
              onOpenSetup={onOpenSetup}
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
              density="compact"
              elevation="none"
              status={composerStatus}
              footerActions={
                <HStack gap={1} vAlign="center" wrap="wrap">
                  <Selector
                    label="Provider"
                    isLabelHidden
                    aria-label="Provider"
                    options={["anthropic", "openai", "openrouter", "qwen"].map((p) => ({
                      value: p,
                      label: p,
                      description: keyStatus[p] ? "configured" : "no key",
                    }))}
                    value={providerId}
                    onChange={(p) => { modelQueryRef.current = ""; setProviderId(p); setModel(MODEL_OPTIONS[p][0]); }}
                    size="md"
                    variant="secondary"
                    width={128}
                  />
                  <Typeahead
                    label="Model"
                    isLabelHidden
                    aria-label="Model"
                    searchSource={modelSearchSource}
                    value={selectedModel}
                    onChange={(item) => {
                      if (!item) return;
                      modelQueryRef.current = "";
                      setModel(item.id);
                    }}
                    onChangeQuery={(query) => { modelQueryRef.current = query; }}
                    onBlur={() => {
                      const typed = modelQueryRef.current.trim();
                      if (typed) setModel(typed);
                    }}
                    renderItem={(item) => item.auxiliaryData?.custom ? `Use "${item.label}"` : item.label}
                    placeholder="Search or type model…"
                    emptySearchText="Type an exact model ID"
                    hasEntriesOnFocus
                    hasClear={false}
                    maxMenuItems={8}
                    debounceMs={0}
                    size="md"
                    width={220}
                  />
                  <Selector
                    label="Approval mode"
                    isLabelHidden
                    aria-label="Approval mode"
                    options={["ask", "auto-edit", "full-auto"]}
                    value={approvalMode}
                    onChange={setApprovalMode}
                    labelTooltip={APPROVAL_MODE_HINTS[approvalMode]}
                    size="md"
                    variant="secondary"
                    width={132}
                  />
                </HStack>
              }
              sendButton={
                running ? (
                  <Button
                    label="Stop"
                    variant="destructive"
                    size="md"
                    icon={<Icon icon={Square} size="sm" />}
                    onClick={() => invoke("cancel_run").catch(() => {})}
                  />
                ) : projectRoot && !providerHasKey ? (
                  <Button
                    label="Connect provider"
                    variant="primary"
                    size="md"
                    icon={<Icon icon={KeyRound} size="sm" />}
                    onClick={onOpenSetup}
                  />
                ) : (
                  <Button
                    label={gate?.needs_confirmation ? "Confirm & run" : "Run"}
                    variant="primary"
                    size="md"
                    icon={<Icon icon={Send} size="sm" />}
                    isDisabled={!ready || !input.trim() || gateBlocked}
                    onClick={() => void run()}
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
    </VStack>
  );
}
