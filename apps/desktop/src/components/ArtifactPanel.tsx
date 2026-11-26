import type { StepId } from "../types";

const ARTIFACT_FOR_STEP: Partial<Record<StepId, string>> = {
  constitution: ".specify/memory/constitution.md",
  specify: "spec.md",
  plan: "plan.md",
  tasks: "tasks.md",
  checklist: "checklists/",
};

export function ArtifactPanel({
  step,
  projectRoot,
  featureSlug,
}: {
  step: StepId;
  projectRoot: string | null;
  featureSlug: string | null;
}) {
  const artifact = ARTIFACT_FOR_STEP[step];
  return (
    <aside className="flex w-96 flex-col border-l border-edge bg-surface-raised">
      <header className="flex items-center justify-between border-b border-edge px-4 py-3">
        <h2 className="text-sm font-medium">
          {artifact ? (
            <>
              Artifact: <code className="font-mono text-xs text-accent">{artifact}</code>
            </>
          ) : (
            "Artifacts"
          )}
        </h2>
      </header>
      <div className="flex-1 overflow-y-auto px-4 py-4 text-sm text-ink-faint">
        {!projectRoot ? (
          <p>No project open.</p>
        ) : (
          <div className="space-y-2 text-xs">
            <p>
              Project: <span className="font-mono text-ink-dim">{projectRoot}</span>
            </p>
            {featureSlug && (
              <p>
                Feature: <span className="font-mono text-ink-dim">{featureSlug}</span>
              </p>
            )}
            <p className="text-ink-faint">
              Live artifact rendering with edit + approve controls lands in M4 (TASK-17).
              Artifacts are being written to disk by run_step already — check
              <code className="mx-1 rounded bg-surface-overlay px-1 font-mono">specs/</code>
              in your project.
            </p>
          </div>
        )}
      </div>
    </aside>
  );
}
