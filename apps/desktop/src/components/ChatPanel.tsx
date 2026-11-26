import { useState } from "react";
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

export function ChatPanel({ step }: { step: StepId }) {
  const [input, setInput] = useState("");
  return (
    <section className="flex flex-1 flex-col min-w-0">
      <header className="flex items-center justify-between border-b border-edge px-5 py-3">
        <div>
          <h1 className="text-sm font-medium capitalize">{step}</h1>
          <p className="text-xs text-ink-faint">{STEP_HINTS[step]}</p>
        </div>
      </header>
      <div className="flex-1 overflow-y-auto px-5 py-6">
        <div className="mx-auto max-w-2xl text-center text-ink-faint text-sm">
          <p className="mb-2 text-4xl">🌱</p>
          <p>
            Open a project to start the speckit pipeline, or run{" "}
            <code className="rounded bg-surface-overlay px-1.5 py-0.5 font-mono text-xs text-accent">
              /speckit.{step}
            </code>{" "}
            with a description below.
          </p>
        </div>
      </div>
      <footer className="border-t border-edge px-5 py-4">
        <div className="mx-auto max-w-2xl">
          <div className="rounded-xl border border-edge bg-surface-raised focus-within:border-accent">
            <textarea
              value={input}
              onChange={(e) => setInput(e.target.value)}
              rows={3}
              placeholder={`Describe the feature for /speckit.${step}…`}
              className="w-full resize-none bg-transparent px-4 py-3 text-sm outline-none placeholder:text-ink-faint"
            />
            <div className="flex items-center justify-between px-3 pb-2">
              <span className="text-xs text-ink-faint">
                Provider: <span className="text-ink-dim">not configured</span>
              </span>
              <button
                disabled={!input.trim()}
                className="rounded-lg bg-accent px-4 py-1.5 text-sm font-medium text-white transition-colors hover:bg-accent-hover disabled:opacity-40"
              >
                Run
              </button>
            </div>
          </div>
        </div>
      </footer>
    </section>
  );
}
