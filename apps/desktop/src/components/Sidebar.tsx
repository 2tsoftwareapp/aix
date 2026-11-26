import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Scroll, Sprout, SearchCheck, Map, ClipboardCheck, ListTodo,
  ScanSearch, Hammer, RefreshCcw, Check, Lock, FolderOpen,
  Settings2, CircleCheck, CircleDot, FileText,
} from "lucide-react";
import { ScrollArea } from "./ui/scroll-area";
import { Tooltip, TooltipContent, TooltipTrigger } from "./ui/tooltip";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { cn } from "../lib/utils";
import { getRecents } from "../lib/recents";
import type { PipelineState, StepId } from "../types";

const STEP_ICONS: Record<StepId, typeof Scroll> = {
  constitution: Scroll,
  specify: Sprout,
  clarify: SearchCheck,
  plan: Map,
  checklist: ClipboardCheck,
  tasks: ListTodo,
  analyze: ScanSearch,
  implement: Hammer,
  converge: RefreshCcw,
};

const STEPS: { id: StepId; label: string; gate?: boolean; gateHint?: string }[] = [
  { id: "constitution", label: "Constitution" },
  { id: "specify", label: "Specify" },
  { id: "clarify", label: "Clarify", gate: true, gateHint: "Requires spec.md — resolves ambiguity before planning" },
  { id: "plan", label: "Plan" },
  { id: "checklist", label: "Checklist", gate: true, gateHint: "Reviewer-owned quality gates — agent cannot self-approve" },
  { id: "tasks", label: "Tasks" },
  { id: "analyze", label: "Analyze", gate: true, gateHint: "Read-only cross-artifact consistency check" },
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
      return "none";
    case "implement": {
      if (!f) return "none";
      const [total, done] = f.artifacts.task_progress;
      if (total === 0) return "none";
      return done >= total ? "done" : done > 0 ? "partial" : "none";
    }
    case "converge":
      return "none";
  }
}

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <div className="px-3 pb-1 pt-3 text-[10px] font-medium uppercase tracking-wider text-muted-foreground/70">
      {children}
    </div>
  );
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
    <aside className="flex h-full flex-col bg-card/50">
      <div className="flex h-12 shrink-0 items-center gap-2 border-b border-border px-4">
        <span className="text-base font-semibold text-primary">aix</span>
        <span className="text-[11px] text-muted-foreground">speckit agent</span>
      </div>

      <ScrollArea className="flex-1">
        <SectionLabel>Project</SectionLabel>
        <div className="px-2 pb-1">
          {projectRoot ? (
            <div className="flex items-start gap-2 rounded-md px-2 py-1.5">
              <FolderOpen className="mt-0.5 h-3.5 w-3.5 shrink-0 text-primary" />
              <div className="min-w-0">
                <p className="truncate text-xs font-medium" title={projectRoot}>
                  {projectRoot.split("/").filter(Boolean).pop()}
                </p>
                <p className="truncate font-mono text-[10px] text-muted-foreground/70" title={projectRoot}>
                  {projectRoot}
                </p>
              </div>
            </div>
          ) : null}
          <Button
            variant="outline"
            size="sm"
            className="w-full justify-start"
            onClick={async () => {
              try {
                const dir = await open({ directory: true, title: "Open project" });
                if (typeof dir === "string") onOpenProject(dir);
              } catch {
                setEditing(true);
              }
            }}
          >
            <FolderOpen className="h-3.5 w-3.5" />
            {projectRoot ? "Change project…" : "Open project…"}
          </Button>
          {!projectRoot && getRecents().length > 0 && (
            <div className="mt-2">
              {getRecents().map((p) => (
                <button
                  key={p}
                  onClick={() => onOpenProject(p)}
                  className="flex w-full items-center gap-2 rounded-md px-2 py-1 text-left text-[11px] text-muted-foreground transition-colors hover:bg-secondary/60 hover:text-foreground"
                  title={p}
                >
                  <FolderOpen className="h-3 w-3 shrink-0 opacity-50" />
                  <span className="truncate font-mono">{p.split("/").filter(Boolean).pop()}</span>
                  <span className="truncate font-mono text-[9px] text-muted-foreground/40 flex-1">{p}</span>
                </button>
              ))}
            </div>
          )}
          {editing && (
            <form
              className="mt-1.5"
              onSubmit={(e) => {
                e.preventDefault();
                if (pathInput.trim()) {
                  onOpenProject(pathInput.trim());
                  setEditing(false);
                }
              }}
            >
              <Input
                value={pathInput}
                onChange={(e) => setPathInput(e.target.value)}
                placeholder="/path/to/project"
                className="h-7 text-xs"
                autoFocus
              />
            </form>
          )}
        </div>

        {pipeline && pipeline.features.length > 0 && (
          <>
            <SectionLabel>Features</SectionLabel>
            <div className="px-2 pb-1">
              {pipeline.features.map((f) => {
                const [total, done] = f.artifacts.task_progress;
                const pct = total > 0 ? Math.round((done / total) * 100) : 0;
                return (
                  <button
                    key={f.slug}
                    onClick={() => onSelectFeature(f.slug)}
                    className={cn(
                      "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs transition-colors",
                      f.slug === activeFeature
                        ? "bg-secondary text-foreground"
                        : "text-muted-foreground hover:bg-secondary/60 hover:text-foreground"
                    )}
                  >
                    {pct === 100 && total > 0 ? (
                      <CircleCheck className="h-3.5 w-3.5 shrink-0 text-primary" />
                    ) : (
                      <FileText className="h-3.5 w-3.5 shrink-0 text-muted-foreground/60" />
                    )}
                    <span className="flex-1 truncate font-mono">{f.slug}</span>
                    {total > 0 && (
                      <span className="font-mono text-[10px] text-muted-foreground/70">
                        {done}/{total}
                      </span>
                    )}
                  </button>
                );
              })}
            </div>
          </>
        )}

        <SectionLabel>Pipeline</SectionLabel>
        <nav className="px-2 pb-2">
          {STEPS.map((step, i) => {
            const active = step.id === activeStep;
            const st = stepStatus(step.id, pipeline, activeFeature);
            const Icon = STEP_ICONS[step.id];
            const row = (
              <button
                key={step.id}
                onClick={() => onSelectStep(step.id)}
                className={cn(
                  "flex w-full items-center gap-2.5 rounded-md px-2 py-1.5 text-left text-[13px] transition-colors",
                  active
                    ? "bg-secondary text-foreground"
                    : "text-muted-foreground hover:bg-secondary/60 hover:text-foreground"
                )}
              >
                <span className="relative flex h-5 w-5 shrink-0 items-center justify-center">
                  {st === "done" ? (
                    <span className="flex h-[18px] w-[18px] items-center justify-center rounded-full bg-primary/20">
                      <Check className="h-3 w-3 text-primary" strokeWidth={3} />
                    </span>
                  ) : st === "partial" ? (
                    <span className="flex h-[18px] w-[18px] items-center justify-center rounded-full border border-primary/60">
                      <CircleDot className="h-2.5 w-2.5 text-primary" />
                    </span>
                  ) : (
                    <span
                      className={cn(
                        "flex h-[18px] w-[18px] items-center justify-center rounded-full border font-mono text-[9px]",
                        active ? "border-primary/60 text-primary" : "border-border text-muted-foreground/70"
                      )}
                    >
                      {i + 1}
                    </span>
                  )}
                </span>
                <Icon className={cn("h-3.5 w-3.5 shrink-0", active ? "text-primary" : "text-muted-foreground/60")} />
                <span className="flex-1">{step.label}</span>
                {step.id === "implement" && (() => {
                  const f = pipeline?.features.find((x) => x.slug === activeFeature);
                  const [total, done] = f?.artifacts.task_progress ?? [0, 0];
                  return total > 0 ? (
                    <span className="font-mono text-[10px] text-muted-foreground/70">{done}/{total}</span>
                  ) : null;
                })()}
                {step.gate && <Lock className="h-3 w-3 shrink-0 text-muted-foreground/50" />}
              </button>
            );
            if (step.gate) {
              return (
                <Tooltip key={step.id}>
                  <TooltipTrigger asChild>{row}</TooltipTrigger>
                  <TooltipContent side="right">{step.gateHint}</TooltipContent>
                </Tooltip>
              );
            }
            return row;
          })}
        </nav>
      </ScrollArea>

      <div className="shrink-0 border-t border-border p-2">
        <Button variant="ghost" size="sm" className="w-full justify-start" onClick={onOpenSetup}>
          <Settings2 className="h-3.5 w-3.5" />
          Provider settings
        </Button>
      </div>
    </aside>
  );
}
