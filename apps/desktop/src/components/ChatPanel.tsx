import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { StepId } from "../types";

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

type StreamPayload =
  | { kind: "delta"; text: string }
  | { kind: "tool_use"; name: string }
  | { kind: "artifact_written"; path: string }
  | { kind: "gate_blocked"; reason: string }
  | { kind: "done" }
  | { kind: "error"; message: string };

interface Msg {
  role: "user" | "assistant" | "system";
  text: string;
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
      } else if (p.kind === "artifact_written") {
        setMessages((prev) => [
          ...prev,
          { role: "system", text: `✓ artifact written: ${p.path}` },
        ]);
      } else if (p.kind === "gate_blocked") {
        setMessages((prev) => [...prev, { role: "system", text: `⛔ gate blocked: ${p.reason}` }]);
        setRunning(false);
      } else if (p.kind === "error") {
        setMessages((prev) => [...prev, { role: "system", text: `⚠ error: ${p.message}` }]);
        setRunning(false);
      } else if (p.kind === "done") {
        setRunning(false);
        onStepDone();
      }
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, [onStepDone]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const run = async () => {
    if (!projectRoot || !input.trim()) return;
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
          provider_id: "anthropic",
          model: "claude-sonnet-4-5",
        },
      });
    } catch (e) {
      setMessages((prev) => [...prev, { role: "system", text: `⚠ invoke failed: ${e}` }]);
      setRunning(false);
    }
  };

  const ready = projectRoot && hasKey;

  return (
    <section className="flex flex-1 flex-col min-w-0">
      <header className="flex items-center justify-between border-b border-edge px-5 py-3">
        <div>
          <h1 className="text-sm font-medium capitalize">{step}</h1>
          <p className="text-xs text-ink-faint">{STEP_HINTS[step]}</p>
        </div>
        {featureSlug && (
          <span className="rounded bg-surface-overlay px-2 py-1 font-mono text-xs text-ink-dim">
            {featureSlug}
          </span>
        )}
      </header>

      <div className="flex-1 overflow-y-auto px-5 py-6">
        <div className="mx-auto max-w-2xl space-y-4">
          {messages.length === 0 && (
            <div className="text-center text-ink-faint text-sm">
              <p className="mb-2 text-4xl">🌱</p>
              {!projectRoot && <p>Open a project folder in the sidebar to begin.</p>}
              {projectRoot && !hasKey && <p>Configure a provider key first.</p>}
              {ready && (
                <p>
                  Describe the input for{" "}
                  <code className="rounded bg-surface-overlay px-1.5 py-0.5 font-mono text-xs text-accent">
                    /speckit.{step}
                  </code>{" "}
                  below.
                </p>
              )}
            </div>
          )}
          {messages.map((m, i) => (
            <div
              key={i}
              className={
                m.role === "user"
                  ? "ml-12 rounded-xl bg-surface-overlay px-4 py-3 text-sm"
                  : m.role === "assistant"
                    ? "mr-12 px-1 py-1 text-sm whitespace-pre-wrap"
                    : "px-1 py-1 font-mono text-xs text-ink-faint"
              }
            >
              {m.text}
            </div>
          ))}
          {running && <div className="text-xs text-ink-faint animate-pulse">streaming…</div>}
          <div ref={bottomRef} />
        </div>
      </div>

      <footer className="border-t border-edge px-5 py-4">
        <div className="mx-auto max-w-2xl">
          <div className="rounded-xl border border-edge bg-surface-raised focus-within:border-accent">
            <textarea
              value={input}
              onChange={(e) => setInput(e.target.value)}
              rows={3}
              disabled={!ready || running}
              placeholder={
                ready ? `Describe the input for /speckit.${step}…` : "Open a project and configure a provider"
              }
              className="w-full resize-none bg-transparent px-4 py-3 text-sm outline-none placeholder:text-ink-faint disabled:opacity-50"
              onKeyDown={(e) => {
                if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) run();
              }}
            />
            <div className="flex items-center justify-between px-3 pb-2">
              <span className="text-xs text-ink-faint">⌘↵ to run</span>
              <button
                onClick={run}
                disabled={!ready || running || !input.trim()}
                className="rounded-lg bg-accent px-4 py-1.5 text-sm font-medium text-white transition-colors hover:bg-accent-hover disabled:opacity-40"
              >
                {running ? "Running…" : "Run"}
              </button>
            </div>
          </div>
        </div>
      </footer>
    </section>
  );
}
