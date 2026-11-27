import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  FolderOpen, MoreHorizontal,
  Settings2, CircleCheck, FileText, LoaderCircle, ShieldQuestion, FolderPlus, Brain } from "lucide-react";
import { SideNav, SideNavItem, SideNavSection, SideNavCollapseButton, useSideNavCollapse } from "@astryxdesign/core/SideNav";
import { HStack, VStack, StackItem } from "@astryxdesign/core/Layout";
import { Text } from "@astryxdesign/core/Text";
import { Icon } from "@astryxdesign/core/Icon";
import { Button } from "@astryxdesign/core/Button";
import { Badge } from "@astryxdesign/core/Badge";
import { DropdownMenu } from "@astryxdesign/core/DropdownMenu";
import { TextInput } from "@astryxdesign/core/TextInput";
import { getRecents } from "../lib/recents";
import { chooseProjectDirectory } from "../lib/projectDialog";
import type { PipelineState } from "../types";
import { useWorkspaces } from "../lib/workspaceStore";
import { AixLogo } from "./brand/AixLogo";

type Feature = PipelineState["features"][number];
type Workspaces = ReturnType<typeof useWorkspaces>;

function featureHasActivity(f: Feature, projectRoot: string | null, workspaces: Workspaces): boolean {
  return Object.values(workspaces).some((workspace) =>
    workspace.projectRoot === projectRoot
    && (workspace.step === "constitution" || workspace.featureSlug === f.slug)
    && (workspace.running || workspace.approvals.length > 0)
  );
}

function orderedFeatures(features: Feature[], activeFeature: string | null, projectRoot: string | null, workspaces: Workspaces): Feature[] {
  const rank = (f: Feature): number => {
    if (f.slug === activeFeature) return 0;
    if (featureHasActivity(f, projectRoot, workspaces)) return 1;
    return 2;
  };
  return [...features].sort((a, b) => rank(a) - rank(b));
}

function FeatureRow({ f, activeFeature, projectRoot, workspaces, onSelectFeature }: {
  f: Feature;
  activeFeature: string | null;
  projectRoot: string | null;
  workspaces: Workspaces;
  onSelectFeature: (slug: string) => void;
}) {
  const [total, done] = f.artifacts.task_progress;
  const complete = total > 0 && done === total;
  return (
    <SideNavItem
      label={f.slug}
      icon={complete ? CircleCheck : FileText}
      isSelected={f.slug === activeFeature}
      onClick={() => onSelectFeature(f.slug)}
      endContent={
        <HStack gap={1} vAlign="center">
          {Object.values(workspaces).some((workspace) =>
            workspace.projectRoot === projectRoot
            && (workspace.step === "constitution" || workspace.featureSlug === f.slug)
            && workspace.running
          ) && <Icon icon={LoaderCircle} size="xsm" color="accent" aria-label="running" />}
          {Object.values(workspaces).some((workspace) =>
            workspace.projectRoot === projectRoot
            && (workspace.step === "constitution" || workspace.featureSlug === f.slug)
            && workspace.approvals.length > 0
          ) && <Icon icon={ShieldQuestion} size="xsm" color="warning" aria-label="approval pending" />}
          <Text type="code" size="2xs" color="secondary">
            {[f.artifacts.spec, f.artifacts.plan, f.artifacts.tasks].filter(Boolean).length}/3 docs{total > 0 ? ` · ${done}/${total}` : ""}
          </Text>
        </HStack>
      }
    />
  );
}

function FeatureItems({ features, activeFeature, projectRoot, workspaces, onSelectFeature }: {
  features: Feature[];
  activeFeature: string | null;
  projectRoot: string | null;
  workspaces: Workspaces;
  onSelectFeature: (slug: string) => void;
}) {
  const collapse = useSideNavCollapse();
  const collapsed = collapse?.isCollapsed ?? false;
  const ordered = orderedFeatures(features, activeFeature, projectRoot, workspaces);
  const cap = collapsed ? 3 : 6;
  const visible = ordered.slice(0, cap);
  const overflow = ordered.slice(cap);
  const rowProps = { activeFeature, projectRoot, workspaces, onSelectFeature };
  return (
    <>
      {visible.map((f) => <FeatureRow key={f.slug} f={f} {...rowProps} />)}
      {overflow.length > 0 && (
        collapsed ? (
          <DropdownMenu
            items={overflow.map((f) => ({
              id: f.slug,
              label: f.slug,
              icon: FileText,
              onClick: () => onSelectFeature(f.slug),
            }))}
            placement="end"
            menuWidth={240}
            renderTrigger={(triggerProps) => (
              <button
                {...triggerProps}
                type="button"
                style={{
                  display: "flex", alignItems: "center", justifyContent: "center",
                  width: "100%", padding: "8px 0", background: "none", border: "none",
                  color: "inherit", cursor: "pointer", outline: "none",
                }}
              >
                <Icon icon={MoreHorizontal} size="sm" color="secondary" />
              </button>
            )}
          />
        ) : (
          <SideNavItem
            label={`${overflow.length} more features`}
            icon={MoreHorizontal}
            collapsible={{ defaultIsCollapsed: true }}
          >
            {overflow.map((f) => <FeatureRow key={f.slug} f={f} {...rowProps} />)}
          </SideNavItem>
        )
      )}
    </>
  );
}

function ProjectRow({ path, isActive, onOpenProject }: {
  path: string;
  isActive: boolean;
  onOpenProject: (path: string) => void;
}) {
  const [isGit, setIsGit] = useState<boolean | null>(null);
  useEffect(() => {
    let cancelled = false;
    invoke<boolean>("is_git_repo", { projectRoot: path })
      .then((v) => { if (!cancelled) setIsGit(v); })
      .catch(() => { if (!cancelled) setIsGit(null); });
    return () => { cancelled = true; };
  }, [path]);
  return (
    <SideNavItem
      label={path.split("/").filter(Boolean).pop() ?? path}
      icon={FolderOpen}
      isSelected={isActive}
      onClick={() => { if (!isActive) onOpenProject(path); }}
      endContent={
        isActive && isGit === false ? (
          <Badge variant="warning" label="no git" />
        ) : (
          <Text type="code" size="2xs" color="secondary" maxLines={1} style={{ maxWidth: 84, overflow: "hidden", textOverflow: "ellipsis", direction: "rtl" }}>{path}</Text>
        )
      }
    />
  );
}

function SidebarFooter({ onOpenSetup, onOpenMemory, memoryEnabled }: {
  onOpenSetup: () => void;
  onOpenMemory: () => void;
  memoryEnabled: boolean;
}) {
  const collapse = useSideNavCollapse();
  const collapsed = collapse?.isCollapsed ?? false;
  if (collapsed) {
    return (
      <VStack padding={2} hAlign="center" gap={1}>
        <Button
          label="Project memory"
          variant="ghost"
          size="sm"
          isIconOnly
          isDisabled={!memoryEnabled}
          icon={<Icon icon={Brain} size="sm" />}
          onClick={onOpenMemory}
        />
        <Button
          label="Provider settings"
          variant="ghost"
          size="sm"
          isIconOnly
          icon={<Icon icon={Settings2} size="sm" />}
          onClick={onOpenSetup}
        />
      </VStack>
    );
  }
  return (
    <VStack padding={2} gap={1}>
      <Button
        label="Project memory"
        variant="ghost"
        size="sm"
        width="100%"
        isDisabled={!memoryEnabled}
        icon={<Icon icon={Brain} size="sm" />}
        onClick={onOpenMemory}
      />
      <Button
        label="Provider settings"
        variant="ghost"
        size="sm"
        width="100%"
        icon={<Icon icon={Settings2} size="sm" />}
        onClick={onOpenSetup}
      />
    </VStack>
  );
}

function BrandHeader() {
  const collapse = useSideNavCollapse();
  const collapsed = collapse?.isCollapsed ?? false;
  if (collapsed) {
    return (
      <VStack gap={2} hAlign="center" padding={2} width="100%">
        <SideNavCollapseButton size="sm" />
      </VStack>
    );
  }
  return (
    <HStack gap={2} vAlign="center" padding={3} width="100%">
      <AixLogo size={30} />
      <VStack gap={0} style={{ minWidth: 0 }}>
        <Text type="large" weight="semibold" maxLines={1}>AIX</Text>
        <Text type="supporting" size="xsm" color="secondary" maxLines={1}>speckit agent</Text>
      </VStack>
      <StackItem size="fill" />
      <SideNavCollapseButton size="sm" />
    </HStack>
  );
}

export function Sidebar({
  projectRoot,
  onOpenProject,
  pipeline,
  activeFeature,
  onSelectFeature,
  onOpenSetup,
  onOpenMemory,
}: {
  projectRoot: string | null;
  onOpenProject: (path: string) => void;
  pipeline: PipelineState | null;
  activeFeature: string | null;
  onSelectFeature: (slug: string) => void;
  onOpenSetup: () => void;
  onOpenMemory: () => void;
}) {
  const [pathInput, setPathInput] = useState("");
  const [editing, setEditing] = useState(false);
  const workspaces = useWorkspaces();

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
      collapsible={{ hasButton: false }}
      header={<BrandHeader />}
      footer={<SidebarFooter onOpenSetup={onOpenSetup} onOpenMemory={onOpenMemory} memoryEnabled={!!projectRoot} />}
    >
      <SideNavSection title="Projects">
        <VStack gap={1} paddingBlockEnd={1}>
          {projectRoot && <ProjectRow path={projectRoot} isActive onOpenProject={onOpenProject} />}
          {getRecents()
            .filter((p) => p !== projectRoot)
            .slice(0, 4)
            .map((p) => (
              <ProjectRow key={p} path={p} isActive={false} onOpenProject={onOpenProject} />
            ))}
          <SideNavItem
            label="Open project…"
            icon={FolderPlus}
            onClick={async () => {
              try {
                const dir = await chooseProjectDirectory();
                if (dir) onOpenProject(dir);
              } catch {
                setEditing(true);
              }
            }}
          />
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
          <FeatureItems
            features={pipeline.features}
            activeFeature={activeFeature}
            projectRoot={projectRoot}
            workspaces={workspaces}
            onSelectFeature={onSelectFeature}
          />
        </SideNavSection>      )}

    </SideNav>
  );
}
