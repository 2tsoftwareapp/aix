import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Pencil, Eye, Copy, Check, FileText, ClipboardCheck, FolderOpen,
} from "lucide-react";
import { HStack, VStack, StackItem } from "@astryxdesign/core/Layout";
import { Text } from "@astryxdesign/core/Text";
import { Icon } from "@astryxdesign/core/Icon";
import { Button } from "@astryxdesign/core/Button";
import { IconButton } from "@astryxdesign/core/IconButton";
import { Badge } from "@astryxdesign/core/Badge";
import { CheckboxInput } from "@astryxdesign/core/CheckboxInput";
import { ProgressBar } from "@astryxdesign/core/ProgressBar";
import { Markdown } from "@astryxdesign/core/Markdown";
import { TabList, Tab } from "@astryxdesign/core/TabList";
import { EmptyState } from "@astryxdesign/core/EmptyState";
import { Spinner } from "@astryxdesign/core/Spinner";
import { TextArea } from "@astryxdesign/core/TextArea";
import { useAppToast } from "../lib/toast";
import { stripMarkdownComments } from "../lib/markdown";
import type { StepId } from "../types";

const ARTIFACT_FOR_STEP: Partial<Record<StepId, string>> = {
  constitution: ".specify/memory/constitution.md",
  specify: "spec.md",
  clarify: "spec.md",
  plan: "plan.md",
  tasks: "tasks.md",
  checklist: "checklists/requirements.md",
  analyze: "tasks.md",
  implement: "tasks.md",
  converge: "tasks.md",
};

const TABS = [
  { rel: "spec.md", label: "spec" },
  { rel: "plan.md", label: "plan" },
  { rel: "tasks.md", label: "tasks" },
];

interface ChecklistItem {
  line: number;
  checked: boolean;
  text: string;
}

interface ArtifactBuffer {
  key: string;
  projectRoot: string;
  featureSlug: string | null;
  relPath: string;
  content: string;
  dirty: boolean;
}

function parseChecklistLine(line: string, index: number): ChecklistItem | null {
  const match = line.match(/^(\s*[-*]\s*)\[( |x|X)\]\s*(.*)$/);
  if (!match) return null;
  return { line: index, checked: match[2].toLowerCase() === "x", text: match[3] };
}


function ArtifactEmptyState({ icon, title, body }: { icon: typeof FileText; title: string; body: string }) {
  return (
    <VStack height="100%" justify="center" align="center" padding={4}>
      <EmptyState
        isCompact
        icon={<Icon icon={icon} size="lg" color="secondary" />}
        title={title}
        description={body}
      />
    </VStack>
  );
}

export function ArtifactPanel({
  step,
  projectRoot,
  featureSlug,
  refreshToken,
  checklists,
}: {
  step: StepId;
  projectRoot: string | null;
  featureSlug: string | null;
  refreshToken: number;
  checklists: string[];
}) {
  const toast = useAppToast();
  const [rel, setRel] = useState<string | null>(ARTIFACT_FOR_STEP[step] ?? "spec.md");
  const [buffer, setBuffer] = useState<ArtifactBuffer | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [editing, setEditing] = useState(false);
  const [copied, setCopied] = useState(false);
  const bufferRef = useRef<ArtifactBuffer | null>(null);
  const draftsRef = useRef(new Map<string, string>());
  const loadGenerationRef = useRef(0);

  useEffect(() => {
    bufferRef.current = buffer;
  }, [buffer]);

  useEffect(() => {
    const forStep = ARTIFACT_FOR_STEP[step];
    if (!forStep) return;
    setRel(step === "checklist" && checklists.length > 0 ? `checklists/${checklists[0]}` : forStep);
  }, [step, checklists]);

  const artifactOwner = useCallback(
    (path: string) => (path.startsWith(".specify") ? null : featureSlug),
    [featureSlug],
  );

  const owner = rel ? artifactOwner(rel) : null;
  const identityKey = projectRoot && rel ? JSON.stringify([projectRoot, owner, rel]) : null;
  const currentBuffer = buffer && buffer.key === identityKey ? buffer : null;
  const content = currentBuffer?.content ?? null;
  const dirty = currentBuffer?.dirty ?? false;

  const stashDirtyBuffer = useCallback(() => {
    const current = bufferRef.current;
    if (current?.dirty) draftsRef.current.set(current.key, current.content);
  }, []);

  useEffect(() => stashDirtyBuffer, [identityKey, stashDirtyBuffer]);

  const load = useCallback(async () => {
    if (!projectRoot || !rel || !identityKey) return;
    const generation = ++loadGenerationRef.current;
    const identity = {
      key: identityKey,
      projectRoot,
      featureSlug: owner,
      relPath: rel,
    };
    setError(null);

    const current = bufferRef.current;
    if (current?.key === identityKey && current.dirty) {
      draftsRef.current.set(current.key, current.content);
    }
    const draft = draftsRef.current.get(identityKey);
    if (draft !== undefined) {
      setBuffer({ ...identity, content: draft, dirty: true });
      return;
    }

    try {
      const text = await invoke<string>("read_artifact", {
        projectRoot,
        featureSlug: owner,
        relPath: rel,
      });
      if (loadGenerationRef.current !== generation) return;
      setBuffer({ ...identity, content: text, dirty: false });
    } catch (e) {
      if (loadGenerationRef.current !== generation) return;
      setBuffer(null);
      setError(String(e));
    }
  }, [projectRoot, owner, rel, identityKey]);

  useEffect(() => {
    void load();
  }, [load, refreshToken]);

  const save = useCallback(async () => {
    const target = bufferRef.current;
    if (!identityKey || !target || target.key !== identityKey || !target.dirty) return;
    setSaving(true);
    try {
      await invoke("write_artifact", {
        projectRoot: target.projectRoot,
        featureSlug: target.featureSlug,
        relPath: target.relPath,
        content: target.content,
      });
      draftsRef.current.delete(target.key);
      setBuffer((current) => {
        if (current?.key !== target.key) return current;
        const stillDirty = current.content !== target.content;
        if (stillDirty) draftsRef.current.set(current.key, current.content);
        return { ...current, dirty: stillDirty };
      });
      toast.success("Artifact saved", target.relPath);
    } catch (e) {
      setError(String(e));
      toast.error("Save failed", String(e));
    } finally {
      setSaving(false);
    }
  }, [identityKey, toast]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "s") {
        e.preventDefault();
        void save();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [save]);

  const copyContent = async () => {
    if (content === null) return;
    try {
      await navigator.clipboard.writeText(content);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch { /* clipboard unavailable */ }
  };

  const isChecklist = rel?.includes("checklists/") ?? false;
  const stepArtifact = ARTIFACT_FOR_STEP[step];
  const artifactTabs = stepArtifact && !TABS.some((tab) => tab.rel === stepArtifact)
    ? [{ rel: stepArtifact, label: step === "constitution" ? "constitution" : "checklist" }, ...TABS]
    : TABS;
  const lines = content?.split("\n") ?? [];
  const checkItems = isChecklist
    ? lines.map(parseChecklistLine).filter((item): item is ChecklistItem => item !== null)
    : [];
  const checkedCount = checkItems.filter((item) => item.checked).length;

  const renderChecklist = () => (
    <VStack height="100%" gap={0}>
      <VStack gap={2} padding={3}>
        <HStack justify="between" vAlign="center" width="100%">
          <HStack gap={1} vAlign="center">
            <Icon icon={ClipboardCheck} size="sm" color="secondary" />
            <Text type="supporting" color="secondary">
              Reviewer checklist — only you can flip these
            </Text>
          </HStack>
          <Text type="code" size="2xs" color="secondary">{checkedCount}/{checkItems.length}</Text>
        </HStack>
        <ProgressBar
          label="Checklist completion"
          isLabelHidden
          value={checkedCount}
          max={Math.max(checkItems.length, 1)}
        />
      </VStack>
      <StackItem size="fill" isScrollable aria-label="Checklist items">
        <VStack gap={1} padding={3}>
          {lines.map((line, index) => {
            const item = parseChecklistLine(line, index);
            if (!item) {
              return line.trim() ? (
                <Text key={index} type="supporting" color="secondary" display="block" textWrap="wrap">
                  {line}
                </Text>
              ) : null;
            }
            return (
              <CheckboxInput
                key={index}
                label={item.text}
                value={item.checked}
                size="sm"
                changeAction={async (checked) => {
                  try {
                    await invoke("flip_checklist_item", {
                      projectRoot,
                      featureSlug,
                      relPath: rel,
                      lineIndex: index,
                      checked,
                    });
                    await load();
                  } catch (err) {
                    toast.error("Flip failed", String(err));
                  }
                }}
              />
            );
          })}
          <HStack justify="end" paddingBlockStart={3}>
            <Button label="← back to artifacts" variant="ghost" size="sm" onClick={() => setRel("spec.md")} />
          </HStack>
        </VStack>
      </StackItem>
    </VStack>
  );

  const renderContent = () => {
    if (!projectRoot) {
      return (
        <ArtifactEmptyState
          icon={FolderOpen}
          title="No project open"
          body="This panel is the review surface: documents the agent writes — spec, plan, tasks, checklists — land here for you to read, edit, and approve."
        />
      );
    }
    if (content !== null && isChecklist) return renderChecklist();
    if (content !== null && editing) {
      return (
        <VStack height="100%" padding={3} isScrollable aria-label="Artifact editor" className="aix-artifact-editor">
          <TextArea
            label="Artifact content"
            isLabelHidden
            value={content}
            onChange={(value) => {
              setBuffer((current) => (
                current?.key === identityKey ? { ...current, content: value, dirty: true } : current
              ));
            }}
            rows={28}
            hasSpellCheck={false}
            width="100%"
          />
        </VStack>
      );
    }
    if (content !== null) {
      return (
        <VStack height="100%" padding={3} isScrollable aria-label="Artifact preview">
          <Markdown density="compact">{stripMarkdownComments(content)}</Markdown>
        </VStack>
      );
    }
    if (error?.includes("no such file") || error?.includes("No such file") || error?.includes("no feature")) {
      return (
        <ArtifactEmptyState
          icon={FileText}
          title={`${rel} doesn't exist yet`}
          body={`Agent output lands here. It will appear after the ${
            rel === "spec.md" ? "specify" : rel === "plan.md" ? "plan" : "tasks"
          } step runs — then you can review and edit it directly.`}
        />
      );
    }
    if (error) {
      return <ArtifactEmptyState icon={FileText} title="Couldn't load artifact" body={error} />;
    }
    return (
      <VStack height="100%" justify="center" align="center">
        <Spinner label="Loading artifact" shade="subtle" />
      </VStack>
    );
  };

  return (
    <VStack height="100%" width="100%" gap={0}>
      <HStack padding={2} vAlign="center" justify="between" width="100%">
        <TabList value={rel ?? ""} onChange={(value) => setRel(value)} size="md" hasDivider>
          {artifactTabs.map((tab) => (
            <Tab key={tab.rel} value={tab.rel} label={tab.label} />
          ))}
        </TabList>
        <HStack gap={1} vAlign="center">
          {content !== null && (
            <>
              <IconButton
                label="Copy content"
                variant="ghost"
                size="sm"
                icon={<Icon icon={copied ? Check : Copy} size="sm" color={copied ? "accent" : "inherit"} />}
                onClick={() => void copyContent()}
              />
              {!isChecklist && (
                <IconButton
                  label={editing ? "Preview" : "Edit"}
                  variant="ghost"
                  size="sm"
                  icon={<Icon icon={editing ? Eye : Pencil} size="sm" />}
                  onClick={() => setEditing((value) => !value)}
                />
              )}
            </>
          )}
          {editing && (
            <Button
              label="Save"
              size="sm"
              variant={dirty ? "primary" : "secondary"}
              isDisabled={!dirty}
              isLoading={saving}
              onClick={() => void save()}
            />
          )}
          {dirty && <Badge variant="warning" label="unsaved" />}
        </HStack>
      </HStack>
      <StackItem size="fill">
        {renderContent()}
      </StackItem>
    </VStack>
  );
}
