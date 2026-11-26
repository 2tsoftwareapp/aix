import type { StepId } from "../types";

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

export function Sidebar({
  activeStep,
  onSelectStep,
}: {
  activeStep: StepId;
  onSelectStep: (s: StepId) => void;
}) {
  return (
    <aside className="flex w-60 flex-col border-r border-edge bg-surface-raised">
      <div className="flex items-center gap-2 px-4 py-3 border-b border-edge">
        <span className="text-accent font-semibold text-lg">aix</span>
        <span className="text-ink-faint text-xs">speckit agent</span>
      </div>
      <div className="px-3 py-2 text-xs uppercase tracking-wide text-ink-faint">
        Project
      </div>
      <button className="mx-2 mb-2 rounded-md border border-dashed border-edge px-3 py-2 text-sm text-ink-dim hover:border-accent hover:text-ink">
        + Open project…
      </button>
      <div className="px-3 py-2 text-xs uppercase tracking-wide text-ink-faint">
        Pipeline
      </div>
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
              <span
                className={`flex h-5 w-5 items-center justify-center rounded-full border text-[10px] ${
                  active ? "border-accent text-accent" : "border-edge text-ink-faint"
                }`}
              >
                {i + 1}
              </span>
              <span className="flex-1">{step.label}</span>
              {step.gate && (
                <span className="text-[10px] text-ink-faint">gate</span>
              )}
            </button>
          );
        })}
      </nav>
      <div className="border-t border-edge px-4 py-3 text-xs text-ink-faint">
        No provider configured
      </div>
    </aside>
  );
}
