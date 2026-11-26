import { useState } from "react";
import {
  Scroll, Sprout, SearchCheck, Map, ClipboardCheck, ListTodo,
  ScanSearch, Hammer, RefreshCcw, Check, Lock, FolderOpen,
  Settings2, CircleCheck, CircleDot, FileText,
} from "lucide-react";
import { SideNav, SideNavItem, SideNavSection } from "@astryxdesign/core/SideNav";
import { HStack, VStack, StackItem } from "@astryxdesign/core/Layout";
import { Text } from "@astryxdesign/core/Text";
import { Icon } from "@astryxdesign/core/Icon";
import { Button } from "@astryxdesign/core/Button";
import { TextInput } from "@astryxdesign/core/TextInput";
import { Card } from "@astryxdesign/core/Card";
import { getRecents } from "../lib/recents";
import { chooseProjectDirectory } from "../lib/projectDialog";
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

function StepEndContent({
  status,
  index,
  gated,
  progress,
}: {
  status: "done" | "partial" | "none";
  index: number;
  gated: boolean;
  progress?: string;
}) {
  return (
    <HStack gap={1} vAlign="center">
      {progress && <Text type="code" size="2xs" color="secondary">{progress}</Text>}
      {gated && <Icon icon={Lock} size="xsm" color="secondary" />}
      {status === "done" ? (
        <Icon icon={Check} size="xsm" color="accent" />
      ) : status === "partial" ? (
        <Icon icon={CircleDot} size="xsm" color="accent" />
      ) : (
        <Text type="code" size="2xs" color="secondary">{index + 1}</Text>
      )}
    </HStack>
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

  const submitPath = () => {
    if (pathInput.trim()) {
      onOpenProject(pathInput.trim());
      setEditing(false);
    }
  };

  return (
    <SideNav
      aria-label="Pipeline navigation"
      resizable={{ defaultWidth: 280, minWidth: 220, maxWidth: 420, autoSaveId: "aix-sidebar-width" }}
      header={
        <HStack gap={2} vAlign="center" padding={3}>
          <Text type="large" weight="semibold" color="accent">aix</Text>
          <Text type="supporting" color="secondary">speckit agent</Text>
        </HStack>
      }
      footer={
        <VStack padding={2}>
          <Button
            label="Provider settings"
            variant="ghost"
            size="sm"
            width="100%"
            icon={<Icon icon={Settings2} size="sm" />}
            onClick={onOpenSetup}
          />
        </VStack>
      }
    >
      <SideNavSection title="Project">
        <VStack gap={2} paddingInline={2} paddingBlockEnd={2}>
          {projectRoot && (
            <Card variant="muted" padding={2}>
              <HStack gap={2} vAlign="start">
                <Icon icon={FolderOpen} size="sm" color="accent" />
                <StackItem size="fill">
                  <VStack gap={0}>
                    <Text type="label" weight="medium" maxLines={1}>
                      {projectRoot.split("/").filter(Boolean).pop()}
                    </Text>
                    <Text type="code" size="2xs" color="secondary" maxLines={1}>
                      {projectRoot}
                    </Text>
                  </VStack>
                </StackItem>
              </HStack>
            </Card>
          )}
          <Button
            label={projectRoot ? "Change project…" : "Open project…"}
            variant="secondary"
            size="sm"
            width="100%"
            icon={<Icon icon={FolderOpen} size="sm" />}
            clickAction={async () => {
              try {
                const dir = await chooseProjectDirectory();
                if (dir) onOpenProject(dir);
              } catch {
                setEditing(true);
              }
            }}
          />
          {!projectRoot && getRecents().length > 0 && (
            <VStack gap={1}>
              {getRecents().map((p) => (
                <SideNavItem
                  key={p}
                  label={p.split("/").filter(Boolean).pop() ?? p}
                  icon={FolderOpen}
                  size="sm"
                  onClick={() => onOpenProject(p)}
                  endContent={<Text type="code" size="2xs" color="secondary" maxLines={1}>{p}</Text>}
                />
              ))}
            </VStack>
          )}
          {editing && (
            <form
              onSubmit={(e) => {
                e.preventDefault();
                submitPath();
              }}
            >
              <TextInput
                label="Project path"
                isLabelHidden
                value={pathInput}
                onChange={setPathInput}
                onEnter={submitPath}
                placeholder="/path/to/project"
                size="sm"
                width="100%"
                hasAutoFocus
              />
            </form>
          )}
        </VStack>
      </SideNavSection>

      {pipeline && pipeline.features.length > 0 && (
        <SideNavSection title="Features">
          {pipeline.features.map((f) => {
            const [total, done] = f.artifacts.task_progress;
            const complete = total > 0 && done === total;
            return (
              <SideNavItem
                key={f.slug}
                label={f.slug}
                icon={complete ? CircleCheck : FileText}
                isSelected={f.slug === activeFeature}
                onClick={() => onSelectFeature(f.slug)}
                endContent={total > 0 ? <Text type="code" size="2xs" color="secondary">{done}/{total}</Text> : undefined}
              />
            );
          })}
        </SideNavSection>
      )}

      <SideNavSection title="Pipeline" subtitle={activeFeature ?? "no feature yet — specify creates the first one"}>
        {STEPS.map((step, i) => {
          const status = stepStatus(step.id, pipeline, activeFeature);
          const feature = pipeline?.features.find((x) => x.slug === activeFeature);
          const [total, done] = feature?.artifacts.task_progress ?? [0, 0];
          return (
            <SideNavItem
              key={step.id}
              label={step.label}
              icon={STEP_ICONS[step.id]}
              isSelected={step.id === activeStep}
              onClick={() => onSelectStep(step.id)}
              aria-label={step.gateHint ?? step.label}
              endContent={
                <StepEndContent
                  status={status}
                  index={i}
                  gated={!!step.gate}
                  progress={step.id === "implement" && total > 0 ? `${done}/${total}` : undefined}
                />
              }
            />
          );
        })}
      </SideNavSection>
    </SideNav>
  );
}
