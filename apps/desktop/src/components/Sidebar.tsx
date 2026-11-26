import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import type { PipelineState, StepId } from "../types";

const STEPS: { id: StepId; label: string; gate?: boolean }[] = [
  { id: "constitution", label: "Constitution" },
  { id: "specify", label: "Specify" },
  { id: "clarify", label: "Clarify", gate: true },
  { id: "plan", label: "Plan" },
  { id: "checklist", label: "Checklist", gate: true },
  { id: "tasks", label: "Tasks" },
  { id: "analyze", label: "Analyze", gate: true },
  { id: "implement", label: "Implement" },
  { id: "converge", label: "Converge" },
];

function stepStatus(
  step: StepId,
  pipeline: PipelineState | null,
  activeFeature: string | null,
): "done" | "partial" | "none" {
  if (!pipeline) return "none";
  const f = pipeline.features.find((x) => x.slug === activeFeature) ?? pipeline.features[pipeline.features.length - 1];
  switch (step) {
    case "constitution":
      return pipeline.has_constitution ? "done" : "none";
    case "specify":
    case "clarify":
      return f?.artifacts.spec ? "done" : "none";
    case "plan":
      return f?.artifacts.plan ? "done" : "none";
    case "checklist":
      if (!f || f.artifacts.checklists.length === 0) return "none";
      return f.artifacts.checklists.every((c) => c.total > 0 && c.checked === c.total) ? "done" : "partial";
    case "tasks":
      return f?.artifacts.tasks ? "done" : "none";
    case "analyze":
      return "none"; // read-only, leaves no artifact
    case "implement": {
      if (!f) return "none";
      const [total, done] = f.artifacts.task_progress;
      if (total === 0) return "none";
      return done >= total ? "done" : done > 0 ? "partial" : "none";
    }
    case "converge":
      return "none"; // verdict parsing lands with M5
  }
}

export function Sidebar({
  activeStep,
  onSelectStep,
  projectRoot,
  onOpenProject,
  pipeline,
  activeFeature,
  onSelectFeature,
  onOpenSetup,
}: {
  activeStep: StepId;
  onSelectStep: (s: StepId) => void;
  projectRoot: string | null;
  onOpenProject: (path: string) => void;
  pipeline: PipelineState | null;
  activeFeature: string | null;
  onSelectFeature: (slug: string) => void;
  onOpenSetup: () => void;
}) {
  const [pathInput, setPathInput] = useState("");
  const [editing, setEditing] = useState(false);

  return (
    <aside className="flex w-60 flex-col border-r border-edge bg-surface-raised">
      <div className="flex items-center gap-2 px-4 py-3 border-b border-edge">
        <span className="text-accent font-semibold text-lg">aix</span>
        <span className="text-ink-faint text-xs">speckit agent</span>
      </div>

      <div className="px-3 py-2 text-xs uppercase tracking-wide text-ink-faint">Project</div>
      <button
        onClick={async () => {
          try {
            const dir = await open({ directory: true, title: "Open project" });
            if (typeof dir === "string") onOpenProject(dir);
          } catch {
            // dialog unavailable (e.g. browser dev) — fall back to typing below
            setEditing(true);
          }
        }}
        className="mx-2 mb-1 rounded-md border border-accent/50 px-3 py-1.5 text-xs text-accent hover:bg-accent/10"
      >
        Open project…
      </button>
      {projectRoot && (
        <p className="mx-2 mb-2 truncate px-1 text-[10px] font-mono text-ink-faint" title={projectRoot}>
          {projectRoot}
        </p>
      )}
      {editing && (
        <form
          className="mx-2 mb-2"
          onSubmit={(e) => {
            e.preventDefault();
            if (pathInput.trim()) {
              onOpenProject(pathInput.trim());
              setEditing(false);
            }
          }}
        >
          <input
            value={pathInput}
            onChange={(e) => setPathInput(e.target.value)}
            placeholder="/path/to/project"
            className="w-full rounded-md border border-dashed border-edge bg-surface px-3 py-2 text-xs outline-none focus:border-accent placeholder:text-ink-faint"
          />
        </form>
      )}

      {pipeline && pipeline.features.length > 0 && (
        <>
          <div className="px-3 py-1 text-xs uppercase tracking-wide text-ink-faint">Features</div>
          <div className="px-2 mb-2">
            {pipeline.features.map((f) => (
              <button
                key={f.slug}
                onClick={() => onSelectFeature(f.slug)}
                className={`block w-full truncate rounded-md px-3 py-1 text-left text-xs ${
                  f.slug === activeFeature ? "bg-surface-overlay text-ink" : "text-ink-dim hover:text-ink"
                }`}
              >
                {f.slug}
                <span className="ml-1 text-ink-faint">
                  {f.artifacts.task_progress[0] > 0 &&
                    `${f.artifacts.task_progress[1]}/${f.artifacts.task_progress[0]}`}
                </span>
              </button>
            ))}
          </div>
        </>
      )}

      <div className="px-3 py-2 text-xs uppercase tracking-wide text-ink-faint">Pipeline</div>
      <nav className="flex-1 overflow-y-auto px-2">
        {STEPS.map((step, i) => {
          const active = step.id === activeStep;
          return (
            <button
              key={step.id}
              onClick={() => onSelectStep(step.id)}
              className={`flex w-full items-center gap-2 rounded-md px-3 py-1.5 text-left text-sm transition-colors ${
                active
                  ? "bg-surface-overlay text-ink"
                  : "text-ink-dim hover:bg-surface-overlay/60 hover:text-ink"
              }`}
            >
              {(() => {
                const st = stepStatus(step.id, pipeline, activeFeature);
                if (st === "done") {
                  return (
                    <span className="flex h-5 w-5 items-center justify-center rounded-full bg-accent/20 text-[10px] text-accent">
                      ✓
                    </span>
                  );
                }
                if (st === "partial") {
                  return (
                    <span className="flex h-5 w-5 items-center justify-center rounded-full border border-accent/60 text-[10px] text-accent">
                      ◐
                    </span>
                  );
                }
                return (
                  <span
                    className={`flex h-5 w-5 items-center justify-center rounded-full border text-[10px] ${
                      active ? "border-accent text-accent" : "border-edge text-ink-faint"
                    }`}
                  >
                    {i + 1}
                  </span>
                );
              })()}
              <span className="flex-1">{step.label}</span>
              {step.id === "implement" && (() => {
                const f = pipeline?.features.find((x) => x.slug === activeFeature);
                const [total, done] = f?.artifacts.task_progress ?? [0, 0];
                return total > 0 ? (
                  <span className="font-mono text-[10px] text-ink-faint">{done}/{total}</span>
                ) : null;
              })()}
              {step.gate && <span className="text-[10px] text-ink-faint">gate</span>}
            </button>
          );
        })}
      </nav>
      <button
        onClick={onOpenSetup}
        className="border-t border-edge px-4 py-3 text-left text-xs text-ink-faint hover:text-ink"
      >
        Provider settings…
      </button>
    </aside>
  );
}
