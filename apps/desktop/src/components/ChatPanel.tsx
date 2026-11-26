import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Markdown from "react-markdown";
import {
  Send, Square, Loader2, ShieldQuestion, FileText, CheckCircle2, XCircle,
  Wrench, AlertTriangle, Ban, Sprout, KeyRound, FolderOpen,
} from "lucide-react";
import { Button } from "./ui/button";
import { Badge } from "./ui/badge";
import { Card, CardContent } from "./ui/card";
import { ScrollArea } from "./ui/scroll-area";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./ui/select";
import { Tooltip, TooltipContent, TooltipTrigger } from "./ui/tooltip";
import type { StepId } from "../types";

const MODEL_OPTIONS: Record<string, string[]> = {
  anthropic: ["claude-opus-4-5", "claude-sonnet-4-5", "claude-haiku-4-5"],
  openai: ["gpt-5.2", "gpt-5.2-mini", "o4-mini"],
  openrouter: ["anthropic/claude-sonnet-4-5", "openai/gpt-5.2", "google/gemini-3-pro"],
  qwen: ["qwen3-max", "qwen-plus", "qwen-turbo", "qwen3-coder-plus"],
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
}

function SystemLine({ msg }: { msg: Msg }) {
  const styles = {
    success: "text-primary",
    error: "text-destructive",
    warning: "text-warning",
    tool: "text-muted-foreground",
    info: "text-muted-foreground",
  } as const;
  const Icon =
    msg.variant === "success" ? CheckCircle2
    : msg.variant === "error" ? XCircle
    : msg.variant === "warning" ? AlertTriangle
    : msg.variant === "tool" ? Wrench
    : CheckCircle2;
  return (
    <div className={`flex items-start gap-2 px-1 py-0.5 text-xs font-mono ${styles[msg.variant ?? "info"]}`}>
      <Icon className="mt-0.5 h-3.5 w-3.5 shrink-0" />
      <span className="break-all">{msg.text}</span>
    </div>
  );
}

function ApprovalCard({ req, onAnswer }: { req: ApprovalReq; onAnswer: (id: string, ok: boolean) => void }) {
  const isWrite = req.tool === "write_file";
  return (
    <Card className="border-primary/40 bg-card animate-slide-up">
      <CardContent className="p-4">
        <div className="mb-2 flex items-center gap-2">
          <ShieldQuestion className="h-4 w-4 text-primary" />
          <span className="text-sm font-medium">Approve {req.tool}?</span>
        </div>
        <p className="mb-2 break-all font-mono text-xs text-muted-foreground">{req.detail}</p>
        {req.preview && (
          <div className="mb-3">
            {isWrite && <p className="mb-1 text-[11px] font-medium text-muted-foreground">Diff preview</p>}
            <pre className="max-h-56 overflow-y-auto rounded-md border border-border bg-background p-2.5 font-mono text-[11px] leading-relaxed text-muted-foreground whitespace-pre-wrap break-all">
              {req.preview}
            </pre>
          </div>
        )}
        <div className="flex gap-2">
          <Button size="sm" onClick={() => onAnswer(req.approval_id, true)}>Approve</Button>
          <Button size="sm" variant="outline" onClick={() => onAnswer(req.approval_id, false)}>Deny</Button>
        </div>
      </CardContent>
    </Card>
  );
}

export function ChatPanel({
  step,
  projectRoot,
  featureSlug,
  hasKey,
  onStepDone,
}: {
  step: StepId;
  projectRoot: string | null;
  featureSlug: string | null;
  hasKey: boolean;
  onStepDone: () => void;
}) {
  const [input, setInput] = useState("");
  const [messages, setMessages] = useState<Msg[]>([]);
  const [running, setRunning] = useState(false);
  const [providerId, setProviderId] = useState("anthropic");
  const [model, setModel] = useState("claude-sonnet-4-5");
  const [approvalMode, setApprovalMode] = useState("ask");
  const [providerHasKey, setProviderHasKey] = useState(false);
  const [gate, setGate] = useState<{ allowed: boolean; needs_confirmation: boolean; reason: string | null } | null>(null);

  useEffect(() => {
    if (!projectRoot) return;
    invoke<{ allowed: boolean; needs_confirmation: boolean; reason: string | null }>("check_gate", {
      step,
      projectRoot,
      featureSlug,
    }).then(setGate).catch(() => setGate(null));
  }, [step, projectRoot, featureSlug, running]);

  useEffect(() => {
    invoke<boolean>("has_provider_key", { providerId }).then(setProviderHasKey);
  }, [providerId, hasKey]);

  const bottomRef = useRef<HTMLDivElement>(null);

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
        setRunning(false);
      } else if (p.kind === "done") {
        setRunning(false);
        onStepDone();
      }
    });
    return () => { unlisten.then((f) => f()); };
  }, [onStepDone]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const [approvals, setApprovals] = useState<ApprovalReq[]>([]);
  useEffect(() => {
    const unlisten = listen<{
      kind: string; approval_id: string; tool: string; detail: string; preview: string | null;
    }>("approval-request", (ev) => {
      if (ev.payload.kind === "request") {
        setApprovals((prev) => [
          ...prev,
          { approval_id: ev.payload.approval_id, tool: ev.payload.tool, detail: ev.payload.detail, preview: ev.payload.preview },
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

  const run = async () => {
    if (!projectRoot || !input.trim() || (gate && !gate.allowed)) return;
    setRunning(true);
    setMessages((prev) => [...prev, { role: "user", text: input }]);
    const userInput = input;
    setInput("");
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
        },
      });
    } catch (e) {
      setMessages((prev) => [...prev, { role: "system", variant: "error", text: `invoke failed: ${e}` }]);
      setRunning(false);
    }
  };

  const ready = projectRoot && providerHasKey;
  const gateBlocked = gate != null && !gate.allowed;

  return (
    <section className="flex h-full flex-1 flex-col min-w-0">
      <header className="flex h-14 shrink-0 items-center justify-between border-b border-border px-4">
        <div className="min-w-0">
          <h1 className="text-sm font-medium capitalize">{step}</h1>
          <p className="truncate text-xs text-muted-foreground">{STEP_HINTS[step]}</p>
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          {featureSlug && (
            <Badge variant="secondary" className="font-mono">{featureSlug}</Badge>
          )}
          <Select
            value={providerId}
            onValueChange={(p) => { setProviderId(p); setModel(MODEL_OPTIONS[p][0]); }}
          >
            <SelectTrigger className="w-[104px]"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="anthropic">anthropic</SelectItem>
              <SelectItem value="openai">openai</SelectItem>
              <SelectItem value="openrouter">openrouter</SelectItem>
              <SelectItem value="qwen">qwen</SelectItem>
            </SelectContent>
          </Select>
          <Select value={model} onValueChange={setModel}>
            <SelectTrigger className="w-[170px] font-mono"><SelectValue /></SelectTrigger>
            <SelectContent>
              {MODEL_OPTIONS[providerId].map((m) => (
                <SelectItem key={m} value={m} className="font-mono">{m}</SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Tooltip>
            <TooltipTrigger asChild>
              <div>
                <Select value={approvalMode} onValueChange={setApprovalMode}>
                  <SelectTrigger className="w-[104px]"><SelectValue /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="ask">ask</SelectItem>
                    <SelectItem value="auto-edit">auto-edit</SelectItem>
                    <SelectItem value="full-auto">full-auto</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </TooltipTrigger>
            <TooltipContent>{APPROVAL_MODE_HINTS[approvalMode]}</TooltipContent>
          </Tooltip>
        </div>
      </header>

      {gateBlocked && (
        <div className="flex shrink-0 items-start gap-2 border-b border-destructive/30 bg-destructive/10 px-4 py-2 text-xs text-destructive">
          <Ban className="mt-0.5 h-3.5 w-3.5 shrink-0" />
          <span>{step} is blocked — {gate!.reason}</span>
        </div>
      )}
      {gate?.allowed && gate.needs_confirmation && (
        <div className="flex shrink-0 items-start gap-2 border-b border-warning/30 bg-warning/10 px-4 py-2 text-xs text-warning">
          <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
          <span>{gate.reason}</span>
        </div>
      )}

      <ScrollArea className="flex-1">
        <div className="mx-auto max-w-2xl space-y-3 px-4 py-5">
          {messages.length === 0 && (
            <div className="flex flex-col items-center gap-3 py-16 text-center">
              {!projectRoot && (
                <>
                  <div className="flex h-12 w-12 items-center justify-center rounded-full bg-secondary">
                    <FolderOpen className="h-5 w-5 text-muted-foreground" />
                  </div>
                  <p className="text-sm font-medium">No project open</p>
                  <p className="max-w-xs text-xs text-muted-foreground">
                    Open a project folder in the sidebar to begin the spec-driven pipeline.
                  </p>
                </>
              )}
              {projectRoot && !providerHasKey && (
                <>
                  <div className="flex h-12 w-12 items-center justify-center rounded-full bg-secondary">
                    <KeyRound className="h-5 w-5 text-muted-foreground" />
                  </div>
                  <p className="text-sm font-medium">No API key for {providerId}</p>
                  <p className="max-w-xs text-xs text-muted-foreground">
                    Open provider settings in the sidebar to store a key, or set the environment variable.
                  </p>
                </>
              )}
              {ready && (
                <>
                  <div className="flex h-12 w-12 items-center justify-center rounded-full bg-primary/10">
                    <Sprout className="h-5 w-5 text-primary" />
                  </div>
                  <p className="text-sm font-medium capitalize">Ready for {step}</p>
                  <p className="max-w-xs text-xs text-muted-foreground">
                    Describe the input for{" "}
                    <code className="rounded bg-secondary px-1 py-0.5 font-mono text-[11px] text-primary">
                      /speckit.{step}
                    </code>{" "}
                    in the composer below. ⌘↵ or Ctrl+Enter to run.
                  </p>
                </>
              )}
            </div>
          )}
          {messages.map((m, i) =>
            m.role === "system" ? (
              <SystemLine key={i} msg={m} />
            ) : m.role === "user" ? (
              <div key={i} className="ml-12 rounded-lg bg-secondary px-3.5 py-2.5 text-sm animate-fade-in">
                {m.text}
              </div>
            ) : (
              <div key={i} className="mr-8 artifact-markdown animate-fade-in">
                <Markdown>{m.text}</Markdown>
              </div>
            )
          )}
          {running && (
            <div className="flex items-center gap-2 px-1 py-1 text-xs text-muted-foreground">
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
              streaming…
            </div>
          )}
          {approvals.map((a) => (
            <ApprovalCard key={a.approval_id} req={a} onAnswer={answerApproval} />
          ))}
          <div ref={bottomRef} />
        </div>
      </ScrollArea>

      <footer className="shrink-0 border-t border-border px-4 py-3">
        <div className="mx-auto max-w-2xl">
          <div className="rounded-lg border border-input bg-card transition-colors focus-within:border-primary/50 focus-within:ring-1 focus-within:ring-ring/30">
            <textarea
              value={input}
              onChange={(e) => setInput(e.target.value)}
              rows={3}
              disabled={!ready || running || gateBlocked}
              placeholder={
                gateBlocked
                  ? `${step} is blocked — resolve the prerequisite first`
                  : ready
                    ? `Describe the input for /speckit.${step}…`
                    : "Open a project and configure a provider"
              }
              className="w-full resize-none bg-transparent px-3.5 py-3 text-sm outline-none placeholder:text-muted-foreground/50 disabled:opacity-50"
              onKeyDown={(e) => {
                if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) run();
              }}
            />
            <div className="flex items-center justify-between px-2.5 pb-2">
              <span className="flex items-center gap-1 text-[11px] text-muted-foreground/60">
                <FileText className="h-3 w-3" /> Ctrl+Enter to run
              </span>
              {running ? (
                <Button
                  variant="destructive"
                  size="sm"
                  onClick={() => invoke("cancel_run").catch(() => {})}
                >
                  <Square className="h-3 w-3" /> Stop
                </Button>
              ) : (
                <Button size="sm" disabled={!ready || !input.trim() || gateBlocked} onClick={run}>
                  <Send className="h-3 w-3" /> Run
                </Button>
              )}
            </div>
          </div>
        </div>
      </footer>
    </section>
  );
}
