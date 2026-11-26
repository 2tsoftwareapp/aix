import { Scroll, Sprout, SearchCheck, Map, ClipboardCheck, ListTodo, ScanSearch, Hammer, RefreshCcw } from "lucide-react";
import type { PipelineState, StepId } from "../types";

export const STEP_ICONS: Record<StepId, typeof Scroll> = {
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

export const STEPS: { id: StepId; label: string; gate?: boolean; gateHint?: string }[] = [
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

export function stepStatus(
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
      return f?.artifacts.spec ? "done" : "none";
    case "clarify":
      return "none";
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
