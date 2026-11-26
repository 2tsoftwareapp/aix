import type { StepId } from "../types";

const ARTIFACT_FOR_STEP: Partial<Record<StepId, string>> = {
  specify: "spec.md",
  plan: "plan.md",
  tasks: "tasks.md",
  checklist: "checklists/",
};

export function ArtifactPanel({
  step,
  onClose,
}: {
  step: StepId;
  onClose: () => void;
}) {
  const artifact = ARTIFACT_FOR_STEP[step];
  return (
    <aside className="flex w-96 flex-col border-l border-edge bg-surface-raised">
      <header className="flex items-center justify-between border-b border-edge px-4 py-3">
        <h2 className="text-sm font-medium">
          {artifact ? (
            <>
              Artifact:{" "}
              <code className="font-mono text-xs text-accent">{artifact}</code>
            </>
          ) : (
            "Artifacts"
          )}
        </h2>
        <button
          onClick={onClose}
          className="text-ink-faint hover:text-ink text-lg leading-none"
        >
          ×
        </button>
      </header>
      <div className="flex-1 overflow-y-auto px-4 py-4 text-sm text-ink-faint">
        <p>No project open.</p>
        <p className="mt-2 text-xs">
          Artifacts render here with edit + approve controls once a
          <code className="mx-1 rounded bg-surface-overlay px-1 font-mono">.specify/</code>
          workspace is active.
        </p>
      </div>
    </aside>
  );
}
