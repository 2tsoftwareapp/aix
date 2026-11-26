export type StepId =
  | "constitution"
  | "specify"
  | "clarify"
  | "plan"
  | "checklist"
  | "tasks"
  | "analyze"
  | "implement"
  | "converge";

export interface ChecklistState {
  name: string;
  total: number;
  checked: number;
}

export interface FeatureArtifacts {
  slug: string;
  dir: string;
  artifacts: {
    spec: boolean;
    plan: boolean;
    tasks: boolean;
    checklists: ChecklistState[];
    task_progress: [number, number];
  };
}

export interface PipelineState {
  project_root: string;
  has_constitution: boolean;
  features: FeatureArtifacts[];
}
