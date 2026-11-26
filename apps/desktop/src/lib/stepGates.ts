import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { STEPS } from "./steps";
import type { PipelineState, StepId } from "../types";

export interface StepGate {
  allowed: boolean;
  needs_confirmation: boolean;
  reason: string | null;
}

export function blockedTarget(reason: string | null | undefined): StepId | null {
  if (!reason) return null;
  if (reason.includes("spec.md") || reason.includes("run /speckit.specify")) return "specify";
  if (reason.includes("plan.md")) return "plan";
  if (reason.includes("tasks.md") || reason.includes("run /speckit.tasks")) return "tasks";
  return null;
}

export function useStepGates(
  projectRoot: string | null,
  activeFeature: string | null,
  pipeline: PipelineState | null,
): Record<string, StepGate> {
  const [gates, setGates] = useState<Record<string, StepGate>>({});
  useEffect(() => {
    if (!projectRoot) {
      setGates({});
      return;
    }
    let cancelled = false;
    void Promise.all(STEPS.map(async (step) => {
      try {
        const gate = await invoke<StepGate>("check_gate", {
          step: step.id,
          projectRoot,
          featureSlug: activeFeature,
        });
        return [step.id, gate] as const;
      } catch {
        return null;
      }
    })).then((entries) => {
      if (cancelled) return;
      setGates(Object.fromEntries(entries.flatMap((entry) => entry ? [entry] : [])));
    });
    return () => { cancelled = true; };
  }, [projectRoot, activeFeature, pipeline]);
  return gates;
}
